import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useEffect, useRef, useState } from 'react';
import { ActionCard, ApprovalCard, ScopeChip, SecretHandoff, StopControl, TimelineItem, } from '@snippy/ui';
import { approveAction, checkForUpdate, continueSession, deleteProviderKey, deleteSession, getDefaultProvider, hasProviderKey, hideOverlayHighlight, installUpdate, listSessions, listWindows, rejectAction, setProviderKey, setWorkingDock, startAndStep, stepSession, stopTask, } from './ipc.js';
import styles from './App.module.css';
const SUGGESTED_TASKS = [
    { title: 'Summarize what is visible in this window', tag: 'Observe' },
    { title: 'Collect table items into a draft list', tag: 'Draft' },
    { title: 'Find and focus the main search box', tag: 'Navigate' },
];
export function App() {
    const [theme, setTheme] = useState('dark');
    const [windows, setWindows] = useState([]);
    const [selectedScope, setSelectedScope] = useState(null);
    const [sessions, setSessions] = useState([]);
    const [activeSession, setActiveSession] = useState(null);
    const [pendingApproval, setPendingApproval] = useState(null);
    const [secretReason, setSecretReason] = useState(null);
    const [blockedReason, setBlockedReason] = useState(null);
    const [messages, setMessages] = useState([]);
    const [isStepping, setIsStepping] = useState(false);
    const [task, setTask] = useState('');
    const [followUp, setFollowUp] = useState('');
    const [provider, setProvider] = useState('demo');
    const [apiKeyInput, setApiKeyInput] = useState('');
    const [hasKey, setHasKey] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [errorMsg, setErrorMsg] = useState(null);
    const [availableUpdate, setAvailableUpdate] = useState(null);
    const [isUpdating, setIsUpdating] = useState(false);
    const [updateStatus, setUpdateStatus] = useState(null);
    const stoppedRef = useRef(false);
    const wasSteppingRef = useRef(false);
    useEffect(() => {
        if (wasSteppingRef.current === isStepping)
            return;
        wasSteppingRef.current = isStepping;
        void setWorkingDock(isStepping);
    }, [isStepping]);
    useEffect(() => {
        void (async () => {
            const [winList, sessList, defProvider] = await Promise.all([
                listWindows(),
                listSessions(),
                getDefaultProvider(),
            ]);
            setWindows(winList);
            setSelectedScope(winList[0] ?? null);
            setSessions(sessList);
            setProvider(defProvider);
        })();
        void checkForUpdate()
            .then((info) => {
            if (info)
                setAvailableUpdate(info);
        })
            .catch(() => { });
        const onSync = (e) => {
            if (e.detail) {
                applyOutcome(e.detail.session, e.detail.outcome);
                void refreshSessions();
            }
        };
        window.addEventListener('snippy-session-sync', onSync);
        return () => window.removeEventListener('snippy-session-sync', onSync);
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
            setBlockedReason(null);
            setMessages((prev) => [
                ...prev,
                { role: 'assistant', text: `Needs approval: ${outcome.action.reason} (${outcome.consequence})` },
            ]);
        }
        else if (outcome.outcome === 'awaiting_secret') {
            setSecretReason(outcome.reason);
            setPendingApproval(null);
            setBlockedReason(null);
            setMessages((prev) => [...prev, { role: 'assistant', text: outcome.reason }]);
        }
        else if (outcome.outcome === 'blocked') {
            setBlockedReason(outcome.reason);
            setPendingApproval(null);
            setSecretReason(null);
            setMessages((prev) => [...prev, { role: 'assistant', text: `Stopped: ${outcome.reason}` }]);
        }
        else if (outcome.outcome === 'complete' || outcome.outcome === 'executed') {
            setPendingApproval(null);
            setSecretReason(null);
            setBlockedReason(null);
            if (outcome.summary) {
                setMessages((prev) => [...prev, { role: 'assistant', text: outcome.summary }]);
            }
        }
        else {
            setPendingApproval(null);
            setSecretReason(null);
            setBlockedReason(null);
        }
    };
    const runAutoLoop = async (initial) => {
        let current = initial;
        applyOutcome(current.session, current.outcome);
        let steps = 0;
        while (current.outcome.outcome === 'executed' &&
            !stoppedRef.current &&
            steps < 5) {
            steps += 1;
            current = await stepSession(current.session.session_id, provider);
            if (stoppedRef.current)
                break;
            applyOutcome(current.session, current.outcome);
        }
    };
    const handleStart = async (overrideTask) => {
        const query = (overrideTask ?? task).trim();
        if (!query || !selectedScope)
            return;
        stoppedRef.current = false;
        setIsStepping(true);
        setErrorMsg(null);
        setShowSettings(false);
        setMessages([{ role: 'user', text: query }]);
        try {
            const res = await startAndStep(query, selectedScope, provider);
            await runAutoLoop(res);
            await refreshSessions();
        }
        catch (err) {
            setErrorMsg(String(err));
        }
        finally {
            setIsStepping(false);
        }
    };
    const handleApprove = async () => {
        if (!activeSession || !pendingApproval)
            return;
        stoppedRef.current = false;
        setIsStepping(true);
        setErrorMsg(null);
        try {
            const res = await approveAction(activeSession.session_id, pendingApproval.action.action_id, provider);
            await runAutoLoop(res);
            await refreshSessions();
        }
        catch (err) {
            setErrorMsg(String(err));
        }
        finally {
            setIsStepping(false);
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
        stoppedRef.current = true;
        setIsStepping(false);
        if (!activeSession)
            return;
        const updated = await stopTask(activeSession.session_id);
        setActiveSession(updated);
        setPendingApproval(null);
        setSecretReason(null);
        await refreshSessions();
    };
    const handleReset = () => {
        setIsStepping(false);
        void hideOverlayHighlight();
        setActiveSession(null);
        setPendingApproval(null);
        setSecretReason(null);
        setBlockedReason(null);
        setMessages([]);
        setErrorMsg(null);
        setTask('');
        setFollowUp('');
    };
    const handleFollowUp = async () => {
        const query = followUp.trim();
        if (!query || !activeSession)
            return;
        stoppedRef.current = false;
        setIsStepping(true);
        setErrorMsg(null);
        setFollowUp('');
        setMessages((prev) => [...prev, { role: 'user', text: query }]);
        try {
            const res = await continueSession(activeSession.session_id, query, provider);
            await runAutoLoop(res);
            await refreshSessions();
        }
        catch (err) {
            setErrorMsg(String(err));
        }
        finally {
            setIsStepping(false);
        }
    };
    const handleDeleteSession = async (sessionId) => {
        await deleteSession(sessionId);
        if (activeSession?.session_id === sessionId) {
            handleReset();
        }
        await refreshSessions();
    };
    const handleResumeAfterTakeover = async () => {
        if (!activeSession)
            return;
        stoppedRef.current = false;
        setIsStepping(true);
        setSecretReason(null);
        setErrorMsg(null);
        try {
            const res = await stepSession(activeSession.session_id, provider);
            await runAutoLoop(res);
            await refreshSessions();
        }
        catch (err) {
            setErrorMsg(String(err));
        }
        finally {
            setIsStepping(false);
        }
    };
    const isRunning = activeSession !== null &&
        activeSession.status !== 'ready' &&
        activeSession.status !== 'complete' &&
        activeSession.status !== 'stopped' &&
        activeSession.status !== 'blocked';
    useEffect(() => {
        const onKeyDown = (e) => {
            if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
                e.preventDefault();
                setShowSettings((prev) => !prev);
            }
            else if (e.key === 'Enter' && pendingApproval) {
                e.preventDefault();
                void handleApprove();
            }
            else if (e.key === 'Enter' && secretReason) {
                e.preventDefault();
                void handleResumeAfterTakeover();
            }
            else if (e.key === 'Escape') {
                if (pendingApproval) {
                    void handleReject();
                }
                else if (isRunning) {
                    void handleStop();
                }
                else if (showSettings) {
                    setShowSettings(false);
                }
            }
        };
        window.addEventListener('keydown', onKeyDown);
        return () => window.removeEventListener('keydown', onKeyDown);
    });
    const providerLabel = provider === 'demo'
        ? 'Demo · local mock'
        : `${provider === 'openai'
            ? 'OpenAI'
            : provider === 'gemini_live'
                ? 'Gemini 3.8 Live (Video)'
                : 'Gemini'} · ${hasKey ? 'key stored on this device' : 'no key saved'}`;
    return (_jsx("div", { className: `${styles.shell} ${isStepping ? styles.shellDocked : ''}`, "data-theme": theme, children: _jsxs("main", { className: styles.bar, "aria-label": "Snippy Command Bar", children: [isStepping ? (_jsxs("div", { className: styles.header, "data-tauri-drag-region": true, style: { justifyContent: 'space-between', height: '76px', borderBottom: 0 }, children: [_jsxs("div", { style: { display: 'flex', alignItems: 'center', gap: '12px', minWidth: 0 }, children: [_jsxs("div", { className: styles.pulseWrap, "aria-hidden": "true", children: [_jsx("span", { className: styles.pulseRing }), _jsx("span", { style: {
                                                width: '9px',
                                                height: '9px',
                                                borderRadius: '50%',
                                                background: 'var(--status-safe)',
                                            } })] }), _jsxs("div", { style: { minWidth: 0 }, children: [_jsx("div", { style: { fontSize: '13.5px', fontWeight: 600 }, children: "Snippy is working\u2026" }), _jsx("div", { style: {
                                                fontSize: '12px',
                                                color: 'var(--content-muted)',
                                                overflow: 'hidden',
                                                textOverflow: 'ellipsis',
                                                whiteSpace: 'nowrap',
                                            }, children: selectedScope?.app_name ?? activeSession?.scope.app_name ?? 'Active window' })] })] }), _jsx(StopControl, { onStop: () => void handleStop() })] })) : !activeSession ? (_jsxs(_Fragment, { children: [_jsxs("div", { className: styles.header, "data-tauri-drag-region": true, children: [_jsxs("svg", { width: "26", height: "26", viewBox: "0 0 28 28", fill: "none", stroke: "var(--status-safe)", strokeWidth: "2.2", strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": "true", children: [_jsx("path", { d: "M4 10V6a2 2 0 0 1 2-2h4M18 4h4a2 2 0 0 1 2 2v4M24 18v4a2 2 0 0 1-2 2h-4M10 24H6a2 2 0 0 1-2-2v-4" }), _jsx("circle", { cx: "14", cy: "14", r: "3", fill: "var(--status-safe)", stroke: "none" })] }), _jsx("input", { type: "text", className: styles.taskInput, value: task, onChange: (e) => setTask(e.target.value), onKeyDown: (e) => {
                                        if (e.key === 'Enter')
                                            void handleStart();
                                    }, placeholder: "Tell Snippy what to do on your screen", "aria-label": "Task for Snippy" }), _jsx("button", { type: "button", className: styles.iconBtn, onClick: () => setShowSettings((s) => !s), "aria-expanded": showSettings, children: "Settings" }), _jsx("button", { type: "button", className: styles.iconBtn, disabled: isStepping, style: {
                                        background: 'var(--action-primary)',
                                        color: '#0b1020',
                                        borderColor: 'transparent',
                                        fontWeight: 600,
                                    }, onClick: () => void handleStart(), children: isStepping ? 'Working…' : 'Run ⏎' })] }), _jsxs("div", { className: styles.pillsRow, children: [selectedScope ? _jsx(ScopeChip, { scope: selectedScope }) : null, _jsx("div", { className: styles.pillNeutral, children: "Ask before changes" }), _jsx("div", { style: { flex: 1 } }), _jsxs("div", { style: { display: 'inline-flex', alignItems: 'center', gap: '7px', fontSize: '12px', color: 'var(--content-muted)' }, children: [_jsx("span", { style: {
                                                width: '7px',
                                                height: '7px',
                                                borderRadius: '50%',
                                                background: provider === 'demo' || hasKey ? 'var(--status-safe)' : 'var(--status-caution)',
                                            } }), _jsx("span", { children: providerLabel })] })] })] })) : (_jsxs("div", { className: styles.header, "data-tauri-drag-region": true, children: [pendingApproval ? (_jsx("div", { style: {
                                width: '24px',
                                height: '24px',
                                borderRadius: '50%',
                                border: '2px solid var(--status-caution)',
                                display: 'flex',
                                alignItems: 'center',
                                justifyContent: 'center',
                                flexShrink: 0,
                            }, children: _jsx("div", { style: { width: '8px', height: '8px', borderRadius: '50%', background: 'var(--status-caution)' } }) })) : secretReason ? (_jsx("div", { style: {
                                width: '10px',
                                height: '10px',
                                borderRadius: '50%',
                                background: 'var(--status-secret)',
                                flexShrink: 0,
                            } })) : (_jsxs("div", { className: styles.pulseWrap, children: [isRunning ? _jsx("div", { className: styles.pulseRing }) : null, _jsx("div", { style: { width: '10px', height: '10px', borderRadius: '50%', background: 'var(--status-safe)' } })] })), _jsxs("div", { style: { flex: 1, minWidth: 0 }, children: [_jsx("div", { style: { fontSize: '15px', fontWeight: 600, lineHeight: '20px' }, children: pendingApproval
                                        ? 'Waiting for your approval'
                                        : secretReason
                                            ? 'Your turn: enter sensitive details'
                                            : activeSession.task }), _jsxs("div", { style: { fontSize: '12.5px', color: 'var(--content-muted)', lineHeight: '17px' }, children: [activeSession.scope.app_name, " \u00B7 ", activeSession.status, " \u00B7 ", activeSession.steps.length, " step(s)"] })] }), !isRunning ? (_jsxs("div", { style: { display: 'flex', gap: '8px', alignItems: 'center' }, children: [_jsx("button", { type: "button", className: styles.iconBtn, onClick: () => void hideOverlayHighlight(), children: "Clear drawings" }), _jsx("button", { type: "button", className: styles.iconBtn, onClick: handleReset, children: "\u2190 Back" })] })) : (_jsx(StopControl, { onStop: () => void handleStop() })), _jsx("div", { className: styles.progressTrack, children: _jsx("div", { className: styles.progressFill, style: {
                                    width: activeSession.status === 'complete' ? '100%' : '50%',
                                    background: pendingApproval
                                        ? 'var(--status-caution)'
                                        : secretReason
                                            ? 'var(--status-secret)'
                                            : 'var(--status-safe)',
                                } }) })] })), availableUpdate ? (_jsxs("div", { className: styles.bodyPad, style: {
                        flexDirection: 'row',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        background: 'rgba(61, 219, 176, 0.08)',
                        borderBottom: '1px solid var(--border-subtle)',
                    }, children: [_jsxs("span", { style: { fontSize: '13px' }, children: ["Update available: ", _jsxs("strong", { children: ["v", availableUpdate.version] })] }), _jsx("button", { type: "button", className: styles.iconBtn, disabled: isUpdating, style: {
                                background: 'var(--action-primary)',
                                color: '#0b1020',
                                borderColor: 'transparent',
                                fontWeight: 600,
                            }, onClick: () => {
                                setIsUpdating(true);
                                void installUpdate().catch((err) => {
                                    setErrorMsg(String(err));
                                    setIsUpdating(false);
                                });
                            }, children: isUpdating ? 'Installing…' : 'Install & Restart' })] })) : null, errorMsg ? (_jsx("div", { className: styles.bodyPad, children: _jsx("p", { role: "alert", style: { color: 'var(--status-danger)', margin: 0, fontSize: '13px' }, children: errorMsg }) })) : null, showSettings ? (_jsxs("section", { className: styles.bodyPad, "aria-label": "Settings and History", children: [_jsxs("div", { style: { display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }, children: [_jsx("label", { htmlFor: "provider-select", style: { fontSize: '13px', color: 'var(--content-secondary)' }, children: "Provider" }), _jsxs("select", { id: "provider-select", value: provider, onChange: (e) => setProvider(e.target.value), className: styles.iconBtn, children: [_jsx("option", { value: "demo", children: "Demo (Local Mock)" }), _jsx("option", { value: "gemini_live", children: "Gemini 3.8 Live (Video Stream)" }), _jsx("option", { value: "gemini", children: "Gemini 3.5 Flash-Lite" }), _jsx("option", { value: "openai", children: "OpenAI" })] }), _jsxs("button", { type: "button", className: styles.iconBtn, onClick: () => setTheme(theme === 'dark' ? 'light' : 'dark'), children: ["Theme: ", theme] }), _jsx("button", { type: "button", className: styles.iconBtn, onClick: () => {
                                        setUpdateStatus('Checking…');
                                        void checkForUpdate()
                                            .then((info) => {
                                            if (info) {
                                                setAvailableUpdate(info);
                                                setUpdateStatus(`Update v${info.version} ready`);
                                            }
                                            else {
                                                setUpdateStatus('Up to date');
                                            }
                                        })
                                            .catch((err) => setUpdateStatus(String(err)));
                                    }, children: "Check for updates" }), updateStatus ? (_jsx("span", { style: { fontSize: '12px', color: 'var(--content-muted)' }, children: updateStatus })) : null] }), provider !== 'demo' ? (_jsxs("div", { style: { display: 'flex', gap: '8px', alignItems: 'center' }, children: [_jsx("input", { type: "password", value: apiKeyInput, onChange: (e) => setApiKeyInput(e.target.value), placeholder: hasKey ? 'Key saved (enter to replace)' : `Enter ${provider} API key`, "aria-label": "Provider API key", className: styles.iconBtn, style: { flex: 1, textAlign: 'left', cursor: 'text' } }), _jsx("button", { type: "button", className: styles.iconBtn, onClick: () => {
                                        if (!apiKeyInput.trim())
                                            return;
                                        void setProviderKey(provider, apiKeyInput).then(() => {
                                            setApiKeyInput('');
                                            setHasKey(true);
                                        });
                                    }, children: "Save Key" }), hasKey ? (_jsx("button", { type: "button", className: styles.iconBtn, onClick: () => void deleteProviderKey(provider).then(() => setHasKey(false)), children: "Clear" })) : null] })) : null, _jsxs("div", { children: [_jsx("div", { className: styles.sectionLabel, style: { padding: '4px 0 6px' }, children: "Recent local sessions" }), sessions.length === 0 ? (_jsx("p", { style: { margin: 0, fontSize: '12.5px', color: 'var(--content-muted)' }, children: "No saved sessions yet." })) : (_jsx("ul", { style: { listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: '6px' }, children: sessions.slice(0, 5).map((s) => (_jsxs("li", { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' }, children: [_jsxs("button", { type: "button", className: styles.iconBtn, style: { flex: 1, textAlign: 'left' }, onClick: () => {
                                                    setActiveSession(s);
                                                    setShowSettings(false);
                                                }, children: [s.task, " (", s.status, ")"] }), _jsx("button", { type: "button", className: styles.iconBtn, onClick: () => void handleDeleteSession(s.session_id), "aria-label": `Delete session ${s.task}`, children: "Delete" })] }, s.session_id))) }))] })] })) : null, !isStepping && !activeSession && !showSettings ? (_jsxs("div", { children: [_jsxs("div", { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', paddingRight: '20px' }, children: [_jsx("div", { className: styles.sectionLabel, children: "Point me at a window" }), _jsx("button", { type: "button", className: styles.iconBtn, onClick: () => {
                                        void listWindows().then((list) => {
                                            setWindows(list);
                                            if (!selectedScope && list[0])
                                                setSelectedScope(list[0]);
                                        });
                                    }, children: "Refresh" })] }), windows.slice(0, 4).map((w) => {
                            const selected = selectedScope?.scope_id === w.scope_id;
                            return (_jsxs("button", { type: "button", className: `${styles.windowRow} ${selected ? styles.windowRowSelected : ''}`, onClick: () => setSelectedScope(w), children: [_jsxs("div", { style: { flex: 1, minWidth: 0 }, children: [_jsx("div", { style: { fontSize: '14px', fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }, children: w.window_title }), _jsxs("div", { style: { fontSize: '12px', color: 'var(--content-muted)' }, children: [w.app_name, " \u00B7 ", w.target_kind] })] }), _jsx("span", { style: { fontSize: '12px', fontWeight: 500, color: selected ? 'var(--status-safe)' : 'var(--content-muted)' }, children: selected ? 'In scope' : 'Select' })] }, w.scope_id));
                        }), _jsx("div", { className: styles.sectionLabel, children: "Suggested for this window" }), SUGGESTED_TASKS.map((item) => (_jsxs("button", { type: "button", className: styles.suggestRow, onClick: () => {
                                setTask(item.title);
                                void handleStart(item.title);
                            }, children: [_jsx("span", { style: { flex: 1, fontSize: '14px', fontWeight: 500 }, children: item.title }), _jsx("span", { className: styles.kbd, children: item.tag })] }, item.title)))] })) : null, !isStepping && activeSession ? (_jsxs("div", { className: styles.bodyPad, children: [messages.length > 0 ? (_jsx("div", { "aria-label": "Conversation", style: {
                                display: 'flex',
                                flexDirection: 'column',
                                gap: '8px',
                                maxHeight: '220px',
                                overflowY: 'auto',
                            }, children: messages.map((m, idx) => (_jsx("div", { style: {
                                    alignSelf: m.role === 'user' ? 'flex-end' : 'flex-start',
                                    maxWidth: '85%',
                                    padding: '8px 12px',
                                    borderRadius: '12px',
                                    fontSize: '13.5px',
                                    lineHeight: '19px',
                                    background: m.role === 'user'
                                        ? 'rgba(61, 219, 176, 0.14)'
                                        : 'var(--surface-raised)',
                                    border: m.role === 'user'
                                        ? '1px solid rgba(61, 219, 176, 0.35)'
                                        : '1px solid var(--border-subtle)',
                                    color: 'var(--content-primary)',
                                    whiteSpace: 'pre-wrap',
                                }, children: m.text }, idx))) })) : null, pendingApproval ? (_jsx(ApprovalCard, { action: pendingApproval.action, consequence: pendingApproval.consequence, onApprove: () => void handleApprove(), onTakeOver: () => {
                                setPendingApproval(null);
                                setSecretReason('User takeover requested.');
                            }, onReject: () => void handleReject() })) : null, secretReason ? (_jsx(SecretHandoff, { reason: secretReason, onContinue: () => void handleResumeAfterTakeover(), onStop: () => void handleStop() })) : null, blockedReason ? (_jsxs("div", { role: "alert", style: {
                                padding: 'var(--space-3) var(--space-4)',
                                borderRadius: 'var(--radius-md)',
                                border: '1px solid var(--status-danger)',
                                background: 'rgba(255, 122, 122, 0.1)',
                                color: 'var(--content-primary)',
                                fontSize: '13.5px',
                            }, children: [_jsx("strong", { children: "Stopped safely:" }), " ", blockedReason] })) : null, activeSession.status === 'complete' ? (_jsx(ActionCard, { stepNumber: activeSession.steps.length, verb: "Verified", target: activeSession.scope.window_title, reason: "Task completed within approved scope.", expectedResult: "All steps verified", risk: "observe" })) : null, !isRunning && !pendingApproval && !secretReason ? (_jsxs("div", { style: { display: 'flex', gap: '8px', alignItems: 'center', marginTop: '4px' }, children: [_jsx("input", { type: "text", value: followUp, onChange: (e) => setFollowUp(e.target.value), onKeyDown: (e) => {
                                        if (e.key === 'Enter')
                                            void handleFollowUp();
                                    }, placeholder: "Reply to Snippy or ask for the next step...", "aria-label": "Follow-up message", className: styles.iconBtn, style: { flex: 1, textAlign: 'left', cursor: 'text', height: '36px', padding: '0 12px' } }), _jsx("button", { type: "button", className: styles.iconBtn, disabled: isStepping, style: {
                                        background: 'var(--action-primary)',
                                        color: '#0b1020',
                                        borderColor: 'transparent',
                                        fontWeight: 600,
                                        height: '36px',
                                        padding: '0 14px',
                                    }, onClick: () => void handleFollowUp(), children: isStepping ? 'Working…' : 'Send ⏎' })] })) : null, activeSession.steps.length > 0 ? (_jsxs("div", { children: [_jsx("div", { className: styles.sectionLabel, style: { padding: '4px 0' }, children: "What I just did" }), _jsx("ul", { style: { listStyle: 'none', padding: 0, margin: 0 }, children: activeSession.steps.map((step) => (_jsx(TimelineItem, { step: step }, step.step_number))) })] })) : null] })) : null, _jsxs("footer", { className: styles.footer, children: [_jsxs("div", { style: { display: 'flex', alignItems: 'center', gap: '6px' }, children: [_jsx("span", { className: styles.kbd, children: "\u23CE" }), _jsx("span", { children: "Run" })] }), _jsxs("div", { style: { display: 'flex', alignItems: 'center', gap: '6px' }, children: [_jsx("span", { className: styles.kbd, children: "Ctrl K" }), _jsx("span", { children: "Settings & History" })] }), _jsxs("div", { style: { display: 'flex', alignItems: 'center', gap: '6px' }, children: [_jsx("span", { className: styles.kbd, children: "Esc" }), _jsx("span", { children: "Stop" })] }), _jsx("div", { style: { flex: 1 } }), _jsx("span", { children: "Only the selected window is observed." })] })] }) }));
}

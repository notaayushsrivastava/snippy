import { useEffect, useRef, useState } from 'react';
import type { ApprovedAction, Scope, SessionRecord } from '@snippy/protocol';
import {
  ActionCard,
  ApprovalCard,
  ScopeChip,
  SecretHandoff,
  StopControl,
  TimelineItem,
} from '@snippy/ui';
import {
  approveAction,
  checkForUpdate,
  continueSession,
  deleteProviderKey,
  deleteSession,
  getDefaultProvider,
  hasProviderKey,
  hideOverlayHighlight,
  installUpdate,
  listSessions,
  listWindows,
  rejectAction,
  setProviderKey,
  setWorkingDock,
  startAndStep,
  stepSession,
  stopTask,
  type ProviderKind,
  type StepResponse,
  type UpdateInfo,
} from './ipc.js';
import styles from './App.module.css';

const SUGGESTED_TASKS = [
  { title: 'Summarize what is visible in this window', tag: 'Observe' },
  { title: 'Collect table items into a draft list', tag: 'Draft' },
  { title: 'Find and focus the main search box', tag: 'Navigate' },
];

export function App() {
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');
  const [windows, setWindows] = useState<Scope[]>([]);
  const [selectedScope, setSelectedScope] = useState<Scope | null>(null);
  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [activeSession, setActiveSession] = useState<SessionRecord | null>(null);
  const [pendingApproval, setPendingApproval] = useState<{
    action: ApprovedAction;
    consequence: string;
  } | null>(null);
  const [secretReason, setSecretReason] = useState<string | null>(null);
  const [blockedReason, setBlockedReason] = useState<string | null>(null);
  const [messages, setMessages] = useState<{ role: 'user' | 'assistant'; text: string }[]>([]);
  const [isStepping, setIsStepping] = useState(false);
  const [task, setTask] = useState('');
  const [followUp, setFollowUp] = useState('');
  const [provider, setProvider] = useState<ProviderKind>('demo');
  const [apiKeyInput, setApiKeyInput] = useState('');
  const [hasKey, setHasKey] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [availableUpdate, setAvailableUpdate] = useState<UpdateInfo | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const stoppedRef = useRef(false);
  const wasSteppingRef = useRef(false);

  useEffect(() => {
    if (wasSteppingRef.current === isStepping) return;
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
        if (info) setAvailableUpdate(info);
      })
      .catch(() => {});

    const onSync = (e: CustomEvent<StepResponse>) => {
      if (e.detail) {
        applyOutcome(e.detail.session, e.detail.outcome);
        void refreshSessions();
      }
    };
    window.addEventListener('snippy-session-sync', onSync as EventListener);
    return () => window.removeEventListener('snippy-session-sync', onSync as EventListener);
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

  const applyOutcome = (
    session: SessionRecord,
    outcome: Awaited<ReturnType<typeof startAndStep>>['outcome'],
  ) => {
    setActiveSession(session);
    if (outcome.outcome === 'awaiting_approval') {
      setPendingApproval({ action: outcome.action, consequence: outcome.consequence });
      setSecretReason(null);
      setBlockedReason(null);
      setMessages((prev) => [
        ...prev,
        { role: 'assistant', text: `Needs approval: ${outcome.action.reason} (${outcome.consequence})` },
      ]);
    } else if (outcome.outcome === 'awaiting_secret') {
      setSecretReason(outcome.reason);
      setPendingApproval(null);
      setBlockedReason(null);
      setMessages((prev) => [...prev, { role: 'assistant', text: outcome.reason }]);
    } else if (outcome.outcome === 'blocked') {
      setBlockedReason(outcome.reason);
      setPendingApproval(null);
      setSecretReason(null);
      setMessages((prev) => [...prev, { role: 'assistant', text: `Stopped: ${outcome.reason}` }]);
    } else if (outcome.outcome === 'complete' || outcome.outcome === 'executed') {
      setPendingApproval(null);
      setSecretReason(null);
      setBlockedReason(null);
      if (outcome.summary) {
        setMessages((prev) => [...prev, { role: 'assistant', text: outcome.summary }]);
      }
    } else {
      setPendingApproval(null);
      setSecretReason(null);
      setBlockedReason(null);
    }
  };

  const runAutoLoop = async (initial: StepResponse) => {
    let current = initial;
    applyOutcome(current.session, current.outcome);
    let steps = 0;
    while (
      current.outcome.outcome === 'executed' &&
      !stoppedRef.current &&
      steps < 5
    ) {
      steps += 1;
      current = await stepSession(current.session.session_id, provider);
      if (stoppedRef.current) break;
      applyOutcome(current.session, current.outcome);
    }
  };

  const handleStart = async (overrideTask?: string) => {
    const query = (overrideTask ?? task).trim();
    if (!query || !selectedScope) return;
    stoppedRef.current = false;
    setIsStepping(true);
    setErrorMsg(null);
    setShowSettings(false);
    setMessages([{ role: 'user', text: query }]);
    try {
      const res = await startAndStep(query, selectedScope, provider);
      await runAutoLoop(res);
      await refreshSessions();
    } catch (err) {
      setErrorMsg(String(err));
    } finally {
      setIsStepping(false);
    }
  };

  const handleApprove = async () => {
    if (!activeSession || !pendingApproval) return;
    stoppedRef.current = false;
    setIsStepping(true);
    setErrorMsg(null);
    try {
      const res = await approveAction(
        activeSession.session_id,
        pendingApproval.action.action_id,
        provider,
      );
      await runAutoLoop(res);
      await refreshSessions();
    } catch (err) {
      setErrorMsg(String(err));
    } finally {
      setIsStepping(false);
    }
  };

  const handleReject = async () => {
    if (!activeSession) return;
    const updated = await rejectAction(activeSession.session_id);
    setActiveSession(updated);
    setPendingApproval(null);
    await refreshSessions();
  };

  const handleStop = async () => {
    stoppedRef.current = true;
    setIsStepping(false);
    if (!activeSession) return;
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
    if (!query || !activeSession) return;
    stoppedRef.current = false;
    setIsStepping(true);
    setErrorMsg(null);
    setFollowUp('');
    setMessages((prev) => [...prev, { role: 'user', text: query }]);
    try {
      const res = await continueSession(activeSession.session_id, query, provider);
      await runAutoLoop(res);
      await refreshSessions();
    } catch (err) {
      setErrorMsg(String(err));
    } finally {
      setIsStepping(false);
    }
  };

  const handleDeleteSession = async (sessionId: string) => {
    await deleteSession(sessionId);
    if (activeSession?.session_id === sessionId) {
      handleReset();
    }
    await refreshSessions();
  };

  const handleResumeAfterTakeover = async () => {
    if (!activeSession) return;
    stoppedRef.current = false;
    setIsStepping(true);
    setSecretReason(null);
    setErrorMsg(null);
    try {
      const res = await stepSession(activeSession.session_id, provider);
      await runAutoLoop(res);
      await refreshSessions();
    } catch (err) {
      setErrorMsg(String(err));
    } finally {
      setIsStepping(false);
    }
  };

  const isRunning =
    activeSession !== null &&
    activeSession.status !== 'ready' &&
    activeSession.status !== 'complete' &&
    activeSession.status !== 'stopped' &&
    activeSession.status !== 'blocked';

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setShowSettings((prev) => !prev);
      } else if (e.key === 'Enter' && pendingApproval) {
        e.preventDefault();
        void handleApprove();
      } else if (e.key === 'Enter' && secretReason) {
        e.preventDefault();
        void handleResumeAfterTakeover();
      } else if (e.key === 'Escape') {
        if (pendingApproval) {
          void handleReject();
        } else if (isRunning) {
          void handleStop();
        } else if (showSettings) {
          setShowSettings(false);
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  });

  const providerLabel =
    provider === 'demo'
      ? 'Demo · local mock'
      : `${
          provider === 'openai'
            ? 'OpenAI'
            : provider === 'gemini_live'
              ? 'Gemini 3.8 Live (Video)'
              : 'Gemini'
        } · ${hasKey ? 'key stored on this device' : 'no key saved'}`;

  return (
    <div className={`${styles.shell} ${isStepping ? styles.shellDocked : ''}`} data-theme={theme}>
      <main className={styles.bar} aria-label="Snippy Command Bar">
        {isStepping ? (
          <div
            className={styles.header}
            data-tauri-drag-region
            style={{ justifyContent: 'space-between', height: '76px', borderBottom: 0 }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px', minWidth: 0 }}>
              <div className={styles.pulseWrap} aria-hidden="true">
                <span className={styles.pulseRing} />
                <span
                  style={{
                    width: '9px',
                    height: '9px',
                    borderRadius: '50%',
                    background: 'var(--status-safe)',
                  }}
                />
              </div>
              <div style={{ minWidth: 0 }}>
                <div style={{ fontSize: '13.5px', fontWeight: 600 }}>Snippy is working…</div>
                <div
                  style={{
                    fontSize: '12px',
                    color: 'var(--content-muted)',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}
                >
                  {selectedScope?.app_name ?? activeSession?.scope.app_name ?? 'Active window'}
                </div>
              </div>
            </div>
            <StopControl onStop={() => void handleStop()} />
          </div>
        ) : !activeSession ? (
          <>
            <div className={styles.header} data-tauri-drag-region>
              <svg
                width="26"
                height="26"
                viewBox="0 0 28 28"
                fill="none"
                stroke="var(--status-safe)"
                strokeWidth="2.2"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path d="M4 10V6a2 2 0 0 1 2-2h4M18 4h4a2 2 0 0 1 2 2v4M24 18v4a2 2 0 0 1-2 2h-4M10 24H6a2 2 0 0 1-2-2v-4" />
                <circle cx="14" cy="14" r="3" fill="var(--status-safe)" stroke="none" />
              </svg>
              <input
                type="text"
                className={styles.taskInput}
                value={task}
                onChange={(e) => setTask(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void handleStart();
                }}
                placeholder="Tell Snippy what to do on your screen"
                aria-label="Task for Snippy"
              />
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => setShowSettings((s) => !s)}
                aria-expanded={showSettings}
              >
                Settings
              </button>
              <button
                type="button"
                className={styles.iconBtn}
                disabled={isStepping}
                style={{
                  background: 'var(--action-primary)',
                  color: '#0b1020',
                  borderColor: 'transparent',
                  fontWeight: 600,
                }}
                onClick={() => void handleStart()}
              >
                {isStepping ? 'Working…' : 'Run ⏎'}
              </button>
            </div>

            <div className={styles.pillsRow}>
              {selectedScope ? <ScopeChip scope={selectedScope} /> : null}
              <div className={styles.pillNeutral}>Ask before changes</div>
              <div style={{ flex: 1 }} />
              <div style={{ display: 'inline-flex', alignItems: 'center', gap: '7px', fontSize: '12px', color: 'var(--content-muted)' }}>
                <span
                  style={{
                    width: '7px',
                    height: '7px',
                    borderRadius: '50%',
                    background: provider === 'demo' || hasKey ? 'var(--status-safe)' : 'var(--status-caution)',
                  }}
                />
                <span>{providerLabel}</span>
              </div>
            </div>
          </>
        ) : (
          <div className={styles.header} data-tauri-drag-region>
            {pendingApproval ? (
              <div
                style={{
                  width: '24px',
                  height: '24px',
                  borderRadius: '50%',
                  border: '2px solid var(--status-caution)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  flexShrink: 0,
                }}
              >
                <div style={{ width: '8px', height: '8px', borderRadius: '50%', background: 'var(--status-caution)' }} />
              </div>
            ) : secretReason ? (
              <div
                style={{
                  width: '10px',
                  height: '10px',
                  borderRadius: '50%',
                  background: 'var(--status-secret)',
                  flexShrink: 0,
                }}
              />
            ) : (
              <div className={styles.pulseWrap}>
                {isRunning ? <div className={styles.pulseRing} /> : null}
                <div style={{ width: '10px', height: '10px', borderRadius: '50%', background: 'var(--status-safe)' }} />
              </div>
            )}

            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: '15px', fontWeight: 600, lineHeight: '20px' }}>
                {pendingApproval
                  ? 'Waiting for your approval'
                  : secretReason
                    ? 'Your turn: enter sensitive details'
                    : activeSession.task}
              </div>
              <div style={{ fontSize: '12.5px', color: 'var(--content-muted)', lineHeight: '17px' }}>
                {activeSession.scope.app_name} · {activeSession.status} · {activeSession.steps.length} step(s)
              </div>
            </div>

            {!isRunning ? (
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                <button
                  type="button"
                  className={styles.iconBtn}
                  onClick={() => void hideOverlayHighlight()}
                >
                  Clear drawings
                </button>
                <button type="button" className={styles.iconBtn} onClick={handleReset}>
                  ← Back
                </button>
              </div>
            ) : (
              <StopControl onStop={() => void handleStop()} />
            )}

            <div className={styles.progressTrack}>
              <div
                className={styles.progressFill}
                style={{
                  width: activeSession.status === 'complete' ? '100%' : '50%',
                  background: pendingApproval
                    ? 'var(--status-caution)'
                    : secretReason
                      ? 'var(--status-secret)'
                      : 'var(--status-safe)',
                }}
              />
            </div>
          </div>
        )}

        {availableUpdate ? (
          <div
            className={styles.bodyPad}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              justifyContent: 'space-between',
              background: 'rgba(61, 219, 176, 0.08)',
              borderBottom: '1px solid var(--border-subtle)',
            }}
          >
            <span style={{ fontSize: '13px' }}>
              Update available: <strong>v{availableUpdate.version}</strong>
            </span>
            <button
              type="button"
              className={styles.iconBtn}
              disabled={isUpdating}
              style={{
                background: 'var(--action-primary)',
                color: '#0b1020',
                borderColor: 'transparent',
                fontWeight: 600,
              }}
              onClick={() => {
                setIsUpdating(true);
                void installUpdate().catch((err) => {
                  setErrorMsg(String(err));
                  setIsUpdating(false);
                });
              }}
            >
              {isUpdating ? 'Installing…' : 'Install & Restart'}
            </button>
          </div>
        ) : null}

        {errorMsg ? (
          <div className={styles.bodyPad}>
            <p role="alert" style={{ color: 'var(--status-danger)', margin: 0, fontSize: '13px' }}>
              {errorMsg}
            </p>
          </div>
        ) : null}

        {showSettings ? (
          <section className={styles.bodyPad} aria-label="Settings and History">
            <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
              <label htmlFor="provider-select" style={{ fontSize: '13px', color: 'var(--content-secondary)' }}>
                Provider
              </label>
              <select
                id="provider-select"
                value={provider}
                onChange={(e) => setProvider(e.target.value as ProviderKind)}
                className={styles.iconBtn}
              >
                <option value="demo">Demo (Local Mock)</option>
                <option value="gemini_live">Gemini 3.8 Live (Video Stream)</option>
                <option value="gemini">Gemini 3.5 Flash-Lite</option>
                <option value="openai">OpenAI</option>
              </select>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
              >
                Theme: {theme}
              </button>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => {
                  setUpdateStatus('Checking…');
                  void checkForUpdate()
                    .then((info) => {
                      if (info) {
                        setAvailableUpdate(info);
                        setUpdateStatus(`Update v${info.version} ready`);
                      } else {
                        setUpdateStatus('Up to date');
                      }
                    })
                    .catch((err) => setUpdateStatus(String(err)));
                }}
              >
                Check for updates
              </button>
              {updateStatus ? (
                <span style={{ fontSize: '12px', color: 'var(--content-muted)' }}>{updateStatus}</span>
              ) : null}
            </div>

            {provider !== 'demo' ? (
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                <input
                  type="password"
                  value={apiKeyInput}
                  onChange={(e) => setApiKeyInput(e.target.value)}
                  placeholder={hasKey ? 'Key saved (enter to replace)' : `Enter ${provider} API key`}
                  aria-label="Provider API key"
                  className={styles.iconBtn}
                  style={{ flex: 1, textAlign: 'left', cursor: 'text' }}
                />
                <button
                  type="button"
                  className={styles.iconBtn}
                  onClick={() => {
                    if (!apiKeyInput.trim()) return;
                    void setProviderKey(provider, apiKeyInput).then(() => {
                      setApiKeyInput('');
                      setHasKey(true);
                    });
                  }}
                >
                  Save Key
                </button>
                {hasKey ? (
                  <button
                    type="button"
                    className={styles.iconBtn}
                    onClick={() => void deleteProviderKey(provider).then(() => setHasKey(false))}
                  >
                    Clear
                  </button>
                ) : null}
              </div>
            ) : null}

            <div>
              <div className={styles.sectionLabel} style={{ padding: '4px 0 6px' }}>
                Recent local sessions
              </div>
              {sessions.length === 0 ? (
                <p style={{ margin: 0, fontSize: '12.5px', color: 'var(--content-muted)' }}>
                  No saved sessions yet.
                </p>
              ) : (
                <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: '6px' }}>
                  {sessions.slice(0, 5).map((s) => (
                    <li
                      key={s.session_id}
                      style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' }}
                    >
                      <button
                        type="button"
                        className={styles.iconBtn}
                        style={{ flex: 1, textAlign: 'left' }}
                        onClick={() => {
                          setActiveSession(s);
                          setShowSettings(false);
                        }}
                      >
                        {s.task} ({s.status})
                      </button>
                      <button
                        type="button"
                        className={styles.iconBtn}
                        onClick={() => void handleDeleteSession(s.session_id)}
                        aria-label={`Delete session ${s.task}`}
                      >
                        Delete
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </section>
        ) : null}

        {!isStepping && !activeSession && !showSettings ? (
          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', paddingRight: '20px' }}>
              <div className={styles.sectionLabel}>Point me at a window</div>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => {
                  void listWindows().then((list) => {
                    setWindows(list);
                    if (!selectedScope && list[0]) setSelectedScope(list[0]);
                  });
                }}
              >
                Refresh
              </button>
            </div>
            {windows.slice(0, 4).map((w) => {
              const selected = selectedScope?.scope_id === w.scope_id;
              return (
                <button
                  key={w.scope_id}
                  type="button"
                  className={`${styles.windowRow} ${selected ? styles.windowRowSelected : ''}`}
                  onClick={() => setSelectedScope(w)}
                >
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: '14px', fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {w.window_title}
                    </div>
                    <div style={{ fontSize: '12px', color: 'var(--content-muted)' }}>
                      {w.app_name} · {w.target_kind}
                    </div>
                  </div>
                  <span style={{ fontSize: '12px', fontWeight: 500, color: selected ? 'var(--status-safe)' : 'var(--content-muted)' }}>
                    {selected ? 'In scope' : 'Select'}
                  </span>
                </button>
              );
            })}

            <div className={styles.sectionLabel}>Suggested for this window</div>
            {SUGGESTED_TASKS.map((item) => (
              <button
                key={item.title}
                type="button"
                className={styles.suggestRow}
                onClick={() => {
                  setTask(item.title);
                  void handleStart(item.title);
                }}
              >
                <span style={{ flex: 1, fontSize: '14px', fontWeight: 500 }}>{item.title}</span>
                <span className={styles.kbd}>{item.tag}</span>
              </button>
            ))}
          </div>
        ) : null}

        {!isStepping && activeSession ? (
          <div className={styles.bodyPad}>
            {messages.length > 0 ? (
              <div
                aria-label="Conversation"
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '8px',
                  maxHeight: '220px',
                  overflowY: 'auto',
                }}
              >
                {messages.map((m, idx) => (
                  <div
                    key={idx}
                    style={{
                      alignSelf: m.role === 'user' ? 'flex-end' : 'flex-start',
                      maxWidth: '85%',
                      padding: '8px 12px',
                      borderRadius: '12px',
                      fontSize: '13.5px',
                      lineHeight: '19px',
                      background:
                        m.role === 'user'
                          ? 'rgba(61, 219, 176, 0.14)'
                          : 'var(--surface-raised)',
                      border:
                        m.role === 'user'
                          ? '1px solid rgba(61, 219, 176, 0.35)'
                          : '1px solid var(--border-subtle)',
                      color: 'var(--content-primary)',
                      whiteSpace: 'pre-wrap',
                    }}
                  >
                    {m.text}
                  </div>
                ))}
              </div>
            ) : null}

            {pendingApproval ? (
              <ApprovalCard
                action={pendingApproval.action}
                consequence={pendingApproval.consequence}
                onApprove={() => void handleApprove()}
                onTakeOver={() => {
                  setPendingApproval(null);
                  setSecretReason('User takeover requested.');
                }}
                onReject={() => void handleReject()}
              />
            ) : null}

            {secretReason ? (
              <SecretHandoff
                reason={secretReason}
                onContinue={() => void handleResumeAfterTakeover()}
                onStop={() => void handleStop()}
              />
            ) : null}

            {blockedReason ? (
              <div
                role="alert"
                style={{
                  padding: 'var(--space-3) var(--space-4)',
                  borderRadius: 'var(--radius-md)',
                  border: '1px solid var(--status-danger)',
                  background: 'rgba(255, 122, 122, 0.1)',
                  color: 'var(--content-primary)',
                  fontSize: '13.5px',
                }}
              >
                <strong>Stopped safely:</strong> {blockedReason}
              </div>
            ) : null}

            {activeSession.status === 'complete' ? (
              <ActionCard
                stepNumber={activeSession.steps.length}
                verb="Verified"
                target={activeSession.scope.window_title}
                reason="Task completed within approved scope."
                expectedResult="All steps verified"
                risk="observe"
              />
            ) : null}

            {!isRunning && !pendingApproval && !secretReason ? (
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center', marginTop: '4px' }}>
                <input
                  type="text"
                  value={followUp}
                  onChange={(e) => setFollowUp(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') void handleFollowUp();
                  }}
                  placeholder="Reply to Snippy or ask for the next step..."
                  aria-label="Follow-up message"
                  className={styles.iconBtn}
                  style={{ flex: 1, textAlign: 'left', cursor: 'text', height: '36px', padding: '0 12px' }}
                />
                <button
                  type="button"
                  className={styles.iconBtn}
                  disabled={isStepping}
                  style={{
                    background: 'var(--action-primary)',
                    color: '#0b1020',
                    borderColor: 'transparent',
                    fontWeight: 600,
                    height: '36px',
                    padding: '0 14px',
                  }}
                  onClick={() => void handleFollowUp()}
                >
                  {isStepping ? 'Working…' : 'Send ⏎'}
                </button>
              </div>
            ) : null}

            {activeSession.steps.length > 0 ? (
              <div>
                <div className={styles.sectionLabel} style={{ padding: '4px 0' }}>
                  What I just did
                </div>
                <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
                  {activeSession.steps.map((step) => (
                    <TimelineItem key={step.step_number} step={step} />
                  ))}
                </ul>
              </div>
            ) : null}
          </div>
        ) : null}

        <footer className={styles.footer}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            <span className={styles.kbd}>⏎</span>
            <span>Run</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            <span className={styles.kbd}>Ctrl K</span>
            <span>Settings &amp; History</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            <span className={styles.kbd}>Esc</span>
            <span>Stop</span>
          </div>
          <div style={{ flex: 1 }} />
          <span>Only the selected window is observed.</span>
        </footer>
      </main>
    </div>
  );
}

import { useEffect, useState } from 'react';
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
  deleteProviderKey,
  deleteSession,
  hasProviderKey,
  listSessions,
  listWindows,
  rejectAction,
  setProviderKey,
  startAndStep,
  stepSession,
  stopTask,
  type ProviderKind,
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
  const [task, setTask] = useState('');
  const [provider, setProvider] = useState<ProviderKind>('demo');
  const [apiKeyInput, setApiKeyInput] = useState('');
  const [hasKey, setHasKey] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

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

  const applyOutcome = (
    session: SessionRecord,
    outcome: Awaited<ReturnType<typeof startAndStep>>['outcome'],
  ) => {
    setActiveSession(session);
    if (outcome.outcome === 'awaiting_approval') {
      setPendingApproval({ action: outcome.action, consequence: outcome.consequence });
      setSecretReason(null);
    } else if (outcome.outcome === 'awaiting_secret') {
      setSecretReason(outcome.reason);
      setPendingApproval(null);
    } else {
      setPendingApproval(null);
      setSecretReason(null);
    }
  };

  const handleStart = async (overrideTask?: string) => {
    const query = (overrideTask ?? task).trim();
    if (!query || !selectedScope) return;
    setErrorMsg(null);
    setShowSettings(false);
    try {
      let res = await startAndStep(query, selectedScope, provider);
      if (res.outcome.outcome === 'executed') {
        res = await stepSession(res.session.session_id, provider);
      }
      applyOutcome(res.session, res.outcome);
      await refreshSessions();
    } catch (err) {
      setErrorMsg(String(err));
    }
  };

  const handleApprove = async () => {
    if (!activeSession || !pendingApproval) return;
    setErrorMsg(null);
    try {
      const res = await approveAction(
        activeSession.session_id,
        pendingApproval.action.action_id,
        provider,
      );
      applyOutcome(res.session, res.outcome);
      await refreshSessions();
    } catch (err) {
      setErrorMsg(String(err));
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
    if (!activeSession) return;
    const updated = await stopTask(activeSession.session_id);
    setActiveSession(updated);
    setPendingApproval(null);
    setSecretReason(null);
    await refreshSessions();
  };

  const handleReset = () => {
    setActiveSession(null);
    setPendingApproval(null);
    setSecretReason(null);
    setErrorMsg(null);
    setTask('');
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
    setSecretReason(null);
    setErrorMsg(null);
    try {
      const res = await stepSession(activeSession.session_id, provider);
      applyOutcome(res.session, res.outcome);
      await refreshSessions();
    } catch (err) {
      setErrorMsg(String(err));
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
      : `${provider === 'openai' ? 'OpenAI' : 'Gemini'} · ${hasKey ? 'key stored on this device' : 'no key saved'}`;

  return (
    <div className={styles.shell} data-theme={theme}>
      <main className={styles.bar} aria-label="Snippy Command Bar">
        {!activeSession ? (
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
                style={{
                  background: 'var(--action-primary)',
                  color: '#0b1020',
                  borderColor: 'transparent',
                  fontWeight: 600,
                }}
                onClick={() => void handleStart()}
              >
                Run ⏎
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
              <button type="button" className={styles.iconBtn} onClick={handleReset}>
                New task
              </button>
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
                <option value="openai">OpenAI</option>
                <option value="gemini">Gemini</option>
              </select>
              <button
                type="button"
                className={styles.iconBtn}
                onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
              >
                Theme: {theme}
              </button>
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

        {!activeSession && !showSettings ? (
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

        {activeSession ? (
          <div className={styles.bodyPad}>
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

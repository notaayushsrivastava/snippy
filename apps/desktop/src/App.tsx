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
  deleteSession,
  listSessions,
  listWindows,
  rejectAction,
  startAndStep,
  stepSession,
  stopTask,
} from './ipc.js';

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

  const handleStart = async () => {
    if (!task.trim() || !selectedScope) return;
    let res = await startAndStep(task.trim(), selectedScope);
    if (res.outcome.outcome === 'executed') {
      res = await stepSession(res.session.session_id);
    }
    applyOutcome(res.session, res.outcome);
    await refreshSessions();
  };

  const handleApprove = async () => {
    if (!activeSession || !pendingApproval) return;
    const res = await approveAction(activeSession.session_id, pendingApproval.action.action_id);
    applyOutcome(res.session, res.outcome);
    await refreshSessions();
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

  const handleDeleteSession = async (sessionId: string) => {
    await deleteSession(sessionId);
    if (activeSession?.session_id === sessionId) {
      setActiveSession(null);
      setPendingApproval(null);
    }
    await refreshSessions();
  };

  const isRunning =
    activeSession !== null &&
    activeSession.status !== 'ready' &&
    activeSession.status !== 'complete' &&
    activeSession.status !== 'stopped' &&
    activeSession.status !== 'blocked';

  return (
    <div
      data-theme={theme}
      style={{
        minHeight: '100vh',
        background: 'var(--surface-canvas)',
        color: 'var(--content-primary)',
        fontFamily: 'system-ui, -apple-system, sans-serif',
        padding: 'var(--space-4)',
        display: 'grid',
        gap: 'var(--space-4)',
        gridTemplateRows: 'auto 1fr',
      }}
    >
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 'var(--space-2)' }}>
        <h1 style={{ fontSize: '20px', margin: 0 }}>Snippy</h1>
        <div style={{ display: 'flex', gap: 'var(--space-2)', alignItems: 'center', flexWrap: 'wrap' }}>
          {selectedScope ? <ScopeChip scope={selectedScope} /> : null}
          <button
            type="button"
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
            style={{
              background: 'var(--surface-panel)',
              color: 'var(--content-primary)',
              border: '1px solid var(--border-subtle)',
              borderRadius: 'var(--radius-sm)',
              padding: 'var(--space-1) var(--space-3)',
              cursor: 'pointer',
            }}
          >
            {theme === 'dark' ? 'Light' : 'Dark'}
          </button>
          {isRunning ? <StopControl onStop={() => void handleStop()} /> : null}
        </div>
      </header>

      <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr)', gap: 'var(--space-4)', maxWidth: '560px' }}>
        <section style={{ display: 'grid', gap: 'var(--space-2)' }}>
          <label htmlFor="scope-select" style={{ fontSize: '14px', color: 'var(--content-secondary)' }}>
            Target window
          </label>
          <select
            id="scope-select"
            value={selectedScope?.scope_id ?? ''}
            onChange={(e) => {
              const found = windows.find((w) => w.scope_id === e.target.value) ?? null;
              setSelectedScope(found);
            }}
            style={{
              padding: 'var(--space-2)',
              borderRadius: 'var(--radius-sm)',
              border: '1px solid var(--border-subtle)',
              background: 'var(--surface-panel)',
              color: 'var(--content-primary)',
            }}
          >
            {windows.map((w) => (
              <option key={w.scope_id} value={w.scope_id}>
                {w.app_name} — {w.window_title}
              </option>
            ))}
          </select>

          <div style={{ display: 'flex', gap: 'var(--space-2)' }}>
            <input
              type="text"
              value={task}
              onChange={(e) => setTask(e.target.value)}
              placeholder="Tell Snippy what you need done..."
              aria-label="Task description"
              style={{
                flex: 1,
                fontSize: '16px',
                padding: 'var(--space-2) var(--space-3)',
                borderRadius: 'var(--radius-sm)',
                border: '1px solid var(--border-subtle)',
                background: 'var(--surface-panel)',
                color: 'var(--content-primary)',
              }}
            />
            <button
              type="button"
              onClick={() => void handleStart()}
              style={{
                background: 'var(--button-primary-bg)',
                color: 'var(--white)',
                border: 'none',
                borderRadius: 'var(--radius-sm)',
                padding: 'var(--space-2) var(--space-4)',
                fontWeight: 600,
                cursor: 'pointer',
              }}
            >
              Start
            </button>
          </div>
        </section>

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
            onContinue={() => setSecretReason(null)}
            onStop={() => void handleStop()}
          />
        ) : null}

        {activeSession?.status === 'complete' ? (
          <ActionCard
            stepNumber={activeSession.steps.length}
            verb="Verified"
            target={activeSession.scope.window_title}
            reason="Task completed within approved scope."
            expectedResult="All steps verified"
            risk="observe"
          />
        ) : null}

        {activeSession ? (
          <section aria-label="Inspector">
            <p style={{ fontSize: '14px', color: 'var(--content-secondary)', margin: '0 0 var(--space-2)' }}>
              Only the selected {activeSession.scope.app_name} window is being observed. Status: <strong>{activeSession.status}</strong>
            </p>
            <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 'var(--space-2)' }}>
              {activeSession.steps.map((step) => (
                <TimelineItem key={step.step_number} step={step} />
              ))}
            </ul>
          </section>
        ) : null}

        <section aria-label="Recent sessions">
          <h2 style={{ fontSize: '16px', margin: '0 0 var(--space-2)' }}>Recent local sessions</h2>
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 'var(--space-2)' }}>
            {sessions.map((s) => (
              <li
                key={s.session_id}
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  padding: 'var(--space-2) var(--space-3)',
                  background: 'var(--surface-panel)',
                  border: '1px solid var(--border-subtle)',
                  borderRadius: 'var(--radius-sm)',
                }}
              >
                <button
                  type="button"
                  onClick={() => setActiveSession(s)}
                  style={{
                    background: 'none',
                    border: 'none',
                    color: 'var(--content-primary)',
                    textAlign: 'left',
                    cursor: 'pointer',
                    flex: 1,
                  }}
                >
                  {s.task} ({s.status})
                </button>
                <button
                  type="button"
                  onClick={() => void handleDeleteSession(s.session_id)}
                  aria-label={`Delete session ${s.task}`}
                  style={{
                    background: 'none',
                    border: '1px solid var(--border-subtle)',
                    borderRadius: 'var(--radius-sm)',
                    color: 'var(--content-secondary)',
                    padding: '2px 8px',
                    cursor: 'pointer',
                  }}
                >
                  Delete
                </button>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </div>
  );
}

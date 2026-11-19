import { useState } from 'react';
import type { ApprovedAction, Scope, SessionStatus, TimelineStep } from '@snippy/protocol';
import {
  ActionCard,
  ApprovalCard,
  ScopeChip,
  SecretHandoff,
  StopControl,
  TimelineItem,
} from '@snippy/ui';

const DEFAULT_SCOPE: Scope = {
  scope_id: 'scope-chrome-research',
  target_kind: 'browser_tab',
  app_name: 'Chrome',
  window_title: 'Research Tab',
  process_id: 4242,
};

const MOCK_APPROVAL: ApprovedAction = {
  protocol_version: 1,
  action_id: 'act-1',
  scope_id: DEFAULT_SCOPE.scope_id,
  observation_hash: 'obs-chrome-research-001',
  kind: 'invoke',
  target: {
    observation_hash: 'obs-chrome-research-001',
    semantic_locator: 'submit-btn',
    coordinate_hint: null,
  },
  arguments: {},
  risk: 'external_impact',
  reason: 'Submit search query in Research Tab',
  expected_postcondition: 'Search results page loads',
  expires_at: '2026-11-19T18:05:00Z',
};

export function App() {
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');
  const [task, setTask] = useState('');
  const [status, setStatus] = useState<SessionStatus>('ready');
  const [steps, setSteps] = useState<TimelineStep[]>([]);

  const handleStart = () => {
    if (!task.trim()) return;
    setStatus('awaiting_approval');
    setSteps([
      {
        step_number: 1,
        timestamp: new Date().toISOString(),
        action_summary: 'Focused search input',
        target_label: 'search-input',
        risk: 'navigate',
        observation_hash: 'obs-chrome-research-001',
        policy_decision: 'allow',
        verification: { status: 'confirmed', evidence: 'Search input focused' },
      },
    ]);
  };

  const handleStop = () => {
    setStatus('stopped');
  };

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
      }}
    >
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h1 style={{ fontSize: '20px', margin: 0 }}>Snippy</h1>
        <div style={{ display: 'flex', gap: 'var(--space-2)', alignItems: 'center' }}>
          <ScopeChip scope={DEFAULT_SCOPE} />
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
          {status !== 'ready' && status !== 'complete' && status !== 'stopped' ? (
            <StopControl onStop={handleStop} />
          ) : null}
        </div>
      </header>

      <main style={{ display: 'grid', gap: 'var(--space-4)', maxWidth: '520px' }}>
        <section style={{ display: 'flex', gap: 'var(--space-2)' }}>
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
            onClick={handleStart}
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
        </section>

        {status === 'awaiting_approval' ? (
          <ApprovalCard
            action={MOCK_APPROVAL}
            consequence={MOCK_APPROVAL.expected_postcondition}
            onApprove={() => setStatus('complete')}
            onTakeOver={() => setStatus('awaiting_secret')}
            onReject={() => setStatus('blocked')}
          />
        ) : null}

        {status === 'awaiting_secret' ? (
          <SecretHandoff
            reason="Sensitive field detected. Snippy will not read or enter it."
            onContinue={() => setStatus('complete')}
            onStop={handleStop}
          />
        ) : null}

        {status === 'complete' ? (
          <ActionCard
            stepNumber={2}
            verb="Verified"
            target="Search results"
            reason="Task completed within approved scope."
            expectedResult="Search results page loads"
            risk="observe"
          />
        ) : null}

        <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 'var(--space-2)' }}>
          {steps.map((step) => (
            <TimelineItem key={step.step_number} step={step} />
          ))}
        </ul>
      </main>
    </div>
  );
}

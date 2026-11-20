import { useEffect, useState } from 'react';
import type { ApprovedAction } from '@snippy/protocol';

interface ApprovalEventDetail {
  sessionId: string;
  action: ApprovedAction;
  consequence: string;
}

export function ApprovalModal() {
  const [detail, setDetail] = useState<ApprovalEventDetail | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const onUpdate = (e: CustomEvent<ApprovalEventDetail>) => {
      if (e.detail) {
        setDetail(e.detail);
        setBusy(false);
      }
    };
    window.addEventListener('snippy-approval-update', onUpdate as EventListener);
    return () => window.removeEventListener('snippy-approval-update', onUpdate as EventListener);
  }, []);

  const respond = async (decision: 'approve' | 'takeover' | 'reject') => {
    if (!detail || busy) return;
    setBusy(true);
    const win = window as unknown as {
      __TAURI_INTERNALS__?: {
        invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
      };
    };
    await win.__TAURI_INTERNALS__?.invoke('respond_to_approval', {
      sessionId: detail.sessionId,
      actionId: detail.action.action_id,
      decision,
      provider: 'gemini_live',
    });
  };

  useEffect(() => {
    if (!detail) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        void respond('approve');
      } else if (e.key === 'Escape') {
        e.preventDefault();
        void respond('reject');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  if (!detail) return null;
  const targetLabel = detail.action.target.semantic_locator ?? 'Selected target';

  return (
    <div
      style={{
        width: '100vw',
        height: '100vh',
        boxSizing: 'border-box',
        padding: '20px',
        background: '#14151A',
        border: '1px solid rgba(255, 184, 77, 0.55)',
        borderRadius: '18px',
        boxShadow: '0 24px 64px rgba(8, 10, 16, 0.45)',
        color: '#F3F4F7',
        fontFamily: 'var(--font-sans)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div
        data-tauri-drag-region
        style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'grab' }}
      >
        <span
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            height: '24px',
            padding: '0 10px',
            borderRadius: '999px',
            background: 'rgba(255, 184, 77, 0.16)',
            color: '#FFC76E',
            fontSize: '11.5px',
            fontWeight: 700,
          }}
        >
          Needs approval
        </span>
        <span style={{ fontSize: '12px', color: '#8B90A0' }}>{detail.action.risk}</span>
      </div>

      <div style={{ marginTop: '12px', fontSize: '19px', fontWeight: 600, lineHeight: '24px' }}>
        {detail.action.kind} on {targetLabel}?
      </div>
      <div style={{ marginTop: '6px', fontSize: '13px', lineHeight: '19px', color: '#A7ACB8', flex: 1 }}>
        {detail.consequence} — {detail.action.reason}
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginTop: '14px' }}>
        <button
          type="button"
          disabled={busy}
          onClick={() => void respond('approve')}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: '8px',
            height: '40px',
            padding: '0 16px',
            borderRadius: '11px',
            border: 0,
            background: '#FFB84D',
            color: '#1C1405',
            fontWeight: 600,
            fontSize: '14px',
            cursor: 'pointer',
          }}
        >
          <span>Approve</span>
          <span
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: '11px',
              color: '#5A3F08',
              background: 'rgba(28, 20, 5, 0.12)',
              padding: '2px 6px',
              borderRadius: '5px',
            }}
          >
            ⏎
          </span>
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void respond('takeover')}
          style={{
            height: '40px',
            padding: '0 14px',
            borderRadius: '11px',
            border: '1px solid rgba(255, 255, 255, 0.22)',
            background: 'transparent',
            color: '#F3F4F7',
            fontWeight: 500,
            fontSize: '14px',
            cursor: 'pointer',
          }}
        >
          Take over
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void respond('reject')}
          style={{
            height: '40px',
            padding: '0 12px',
            borderRadius: '11px',
            border: 0,
            background: 'transparent',
            color: '#C9CDD8',
            fontWeight: 500,
            fontSize: '14px',
            cursor: 'pointer',
          }}
        >
          Reject
        </button>
      </div>

      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          marginTop: '14px',
          paddingTop: '12px',
          borderTop: '1px solid rgba(255, 255, 255, 0.08)',
          fontSize: '12px',
          color: '#8B90A0',
        }}
      >
        <span>The overlay is clickable only while I am asking.</span>
      </div>
    </div>
  );
}

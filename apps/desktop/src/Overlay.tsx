import { useEffect, useState } from 'react';

interface OverlayPayload {
  label: string;
  step_number: number;
}

export function Overlay() {
  const [payload, setPayload] = useState<OverlayPayload>({
    label: 'Snippy target',
    step_number: 1,
  });

  useEffect(() => {
    const onMessage = (e: CustomEvent<OverlayPayload>) => {
      if (e.detail) setPayload(e.detail);
    };
    window.addEventListener('snippy-overlay-update', onMessage as EventListener);
    return () => {
      window.removeEventListener('snippy-overlay-update', onMessage as EventListener);
    };
  }, []);

  return (
    <div
      style={{
        width: '100vw',
        height: '100vh',
        boxSizing: 'border-box',
        border: '2px solid var(--status-safe-border)',
        borderRadius: '10px',
        background: 'rgba(15, 163, 127, 0.08)',
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'flex-start',
        padding: '6px',
        pointerEvents: 'none',
      }}
    >
      <div
        style={{
          background: 'var(--ink-950)',
          color: 'var(--text-100)',
          borderRadius: '8px',
          padding: '3px 10px',
          fontSize: '11.5px',
          fontWeight: 600,
          fontFamily: 'var(--font-sans)',
        }}
      >
        #{payload.step_number} {payload.label}
      </div>
    </div>
  );
}

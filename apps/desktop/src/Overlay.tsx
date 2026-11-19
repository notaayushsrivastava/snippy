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
    const win = window as unknown as {
      __TAURI_INTERNALS__?: {
        transformCallback?: unknown;
      };
    };
    const onMessage = (e: CustomEvent<OverlayPayload>) => {
      if (e.detail) setPayload(e.detail);
    };
    window.addEventListener('snippy-overlay-update', onMessage as EventListener);
    void win;
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
        border: '3px solid var(--overlay-outline)',
        borderRadius: 'var(--radius-sm)',
        background: 'transparent',
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'flex-start',
        padding: '4px',
        pointerEvents: 'none',
      }}
    >
      <div
        style={{
          background: 'var(--surface-canvas)',
          color: 'var(--content-primary)',
          border: '1px solid var(--border-subtle)',
          borderRadius: 'var(--radius-sm)',
          padding: '2px 8px',
          fontSize: '12px',
          fontWeight: 600,
          fontFamily: 'system-ui, -apple-system, sans-serif',
        }}
      >
        #{payload.step_number} {payload.label}
      </div>
    </div>
  );
}

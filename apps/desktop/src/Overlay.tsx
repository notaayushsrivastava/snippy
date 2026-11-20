import { useEffect, useState } from 'react';

interface OverlayPayload {
  label: string;
  step_number: number;
}

export interface ScreenAnnotation {
  kind: string;
  x: number;
  y: number;
  width?: number | null;
  height?: number | null;
  x2?: number | null;
  y2?: number | null;
  text?: string | null;
  color?: string | null;
}

export function Overlay() {
  const [payload, setPayload] = useState<OverlayPayload | null>(null);
  const [annotations, setAnnotations] = useState<ScreenAnnotation[]>([]);

  useEffect(() => {
    const onHighlight = (e: CustomEvent<OverlayPayload>) => {
      if (e.detail) {
        setPayload(e.detail);
        setAnnotations([]);
      }
    };
    const onDraw = (e: CustomEvent<ScreenAnnotation[]>) => {
      if (Array.isArray(e.detail)) {
        setAnnotations(e.detail);
        setPayload(null);
      }
    };
    window.addEventListener('snippy-overlay-update', onHighlight as EventListener);
    window.addEventListener('snippy-overlay-draw', onDraw as EventListener);
    return () => {
      window.removeEventListener('snippy-overlay-update', onHighlight as EventListener);
      window.removeEventListener('snippy-overlay-draw', onDraw as EventListener);
    };
  }, []);

  if (annotations.length > 0) {
    return (
      <svg
        width="100vw"
        height="100vh"
        style={{
          position: 'fixed',
          inset: 0,
          width: '100vw',
          height: '100vh',
          pointerEvents: 'none',
          background: 'transparent',
        }}
      >
        <defs>
          <marker
            id="snippy-arrowhead"
            markerWidth="10"
            markerHeight="10"
            refX="8"
            refY="3"
            orient="auto"
          >
            <path d="M0,0 L0,6 L9,3 z" fill="#3DDBB0" />
          </marker>
        </defs>
        {annotations.map((ann, idx) => {
          const stroke = ann.color ?? '#3DDBB0';
          if (ann.kind === 'circle') {
            const r = Math.max(18, (ann.width ?? 60) / 2);
            return (
              <g key={idx}>
                <circle
                  cx={ann.x}
                  cy={ann.y}
                  r={r}
                  fill="rgba(61, 219, 176, 0.1)"
                  stroke={stroke}
                  strokeWidth="3"
                />
                {ann.text ? (
                  <text
                    x={ann.x}
                    y={ann.y - r - 8}
                    fill="#F3F4F7"
                    fontSize="12"
                    fontWeight="600"
                    fontFamily="Inter, system-ui, sans-serif"
                  >
                    {ann.text}
                  </text>
                ) : null}
              </g>
            );
          }
          if (ann.kind === 'arrow' && ann.x2 != null && ann.y2 != null) {
            const cx = Math.round((ann.x + ann.x2) / 2 - 40);
            const cy = Math.round((ann.y + ann.y2) / 2 + 20);
            return (
              <g key={idx}>
                <path
                  d={`M ${ann.x} ${ann.y} Q ${cx} ${cy} ${ann.x2} ${ann.y2}`}
                  fill="none"
                  stroke={stroke}
                  strokeWidth="3"
                  strokeLinecap="round"
                />
                <circle cx={ann.x2} cy={ann.y2} r="5" fill={stroke} />
                {ann.text ? (
                  <text
                    x={ann.x}
                    y={ann.y - 8}
                    fill="#F3F4F7"
                    fontSize="12"
                    fontWeight="600"
                    fontFamily="Inter, system-ui, sans-serif"
                  >
                    {ann.text}
                  </text>
                ) : null}
              </g>
            );
          }
          if (ann.kind === 'label') {
            return (
              <g key={idx} transform={`translate(${ann.x}, ${ann.y})`}>
                <rect
                  x="0"
                  y="0"
                  width={Math.max(90, (ann.text?.length ?? 6) * 8 + 20)}
                  height="28"
                  rx="8"
                  fill="#14151A"
                  stroke={stroke}
                  strokeWidth="1.5"
                />
                <text
                  x="10"
                  y="18"
                  fill="#F3F4F7"
                  fontSize="12"
                  fontWeight="600"
                  fontFamily="Inter, system-ui, sans-serif"
                >
                  {ann.text ?? 'Snippy'}
                </text>
              </g>
            );
          }
          const w = Math.max(40, ann.width ?? 160);
          const h = Math.max(28, ann.height ?? 44);
          return (
            <g key={idx}>
              <rect
                x={ann.x}
                y={ann.y}
                width={w}
                height={h}
                rx="8"
                fill="rgba(15, 163, 127, 0.08)"
                stroke={stroke}
                strokeWidth="2.5"
                strokeDasharray="6 4"
              />
              {ann.text ? (
                <text
                  x={ann.x + 6}
                  y={Math.max(16, ann.y - 6)}
                  fill="#F3F4F7"
                  fontSize="12"
                  fontWeight="600"
                  fontFamily="Inter, system-ui, sans-serif"
                >
                  {ann.text}
                </text>
              ) : null}
            </g>
          );
        })}
      </svg>
    );
  }

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
        #{payload?.step_number ?? 1} {payload?.label ?? 'Snippy target'}
      </div>
    </div>
  );
}

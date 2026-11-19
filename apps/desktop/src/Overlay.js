import { jsxs as _jsxs, jsx as _jsx } from "react/jsx-runtime";
import { useEffect, useState } from 'react';
export function Overlay() {
    const [payload, setPayload] = useState({
        label: 'Snippy target',
        step_number: 1,
    });
    useEffect(() => {
        const onMessage = (e) => {
            if (e.detail)
                setPayload(e.detail);
        };
        window.addEventListener('snippy-overlay-update', onMessage);
        return () => {
            window.removeEventListener('snippy-overlay-update', onMessage);
        };
    }, []);
    return (_jsx("div", { style: {
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
        }, children: _jsxs("div", { style: {
                background: 'var(--ink-950)',
                color: 'var(--text-100)',
                borderRadius: '8px',
                padding: '3px 10px',
                fontSize: '11.5px',
                fontWeight: 600,
                fontFamily: 'var(--font-sans)',
            }, children: ["#", payload.step_number, " ", payload.label] }) }));
}

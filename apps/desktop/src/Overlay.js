import { jsxs as _jsxs, jsx as _jsx } from "react/jsx-runtime";
import { useEffect, useState } from 'react';
export function Overlay() {
    const [payload, setPayload] = useState({
        label: 'Snippy target',
        step_number: 1,
    });
    useEffect(() => {
        const win = window;
        const onMessage = (e) => {
            if (e.detail)
                setPayload(e.detail);
        };
        window.addEventListener('snippy-overlay-update', onMessage);
        void win;
        return () => {
            window.removeEventListener('snippy-overlay-update', onMessage);
        };
    }, []);
    return (_jsx("div", { style: {
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
        }, children: _jsxs("div", { style: {
                background: 'var(--surface-canvas)',
                color: 'var(--content-primary)',
                border: '1px solid var(--border-subtle)',
                borderRadius: 'var(--radius-sm)',
                padding: '2px 8px',
                fontSize: '12px',
                fontWeight: 600,
                fontFamily: 'system-ui, -apple-system, sans-serif',
            }, children: ["#", payload.step_number, " ", payload.label] }) }));
}

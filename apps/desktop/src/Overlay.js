import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useEffect, useState } from 'react';
export function Overlay() {
    const [payload, setPayload] = useState(null);
    const [annotations, setAnnotations] = useState([]);
    useEffect(() => {
        const onHighlight = (e) => {
            if (e.detail) {
                setPayload(e.detail);
                setAnnotations([]);
            }
        };
        const onDraw = (e) => {
            if (Array.isArray(e.detail)) {
                setAnnotations(e.detail);
                setPayload(null);
            }
        };
        window.addEventListener('snippy-overlay-update', onHighlight);
        window.addEventListener('snippy-overlay-draw', onDraw);
        return () => {
            window.removeEventListener('snippy-overlay-update', onHighlight);
            window.removeEventListener('snippy-overlay-draw', onDraw);
        };
    }, []);
    if (annotations.length > 0) {
        return (_jsxs("svg", { width: "100vw", height: "100vh", style: {
                position: 'fixed',
                inset: 0,
                width: '100vw',
                height: '100vh',
                pointerEvents: 'none',
                background: 'transparent',
            }, children: [_jsx("defs", { children: _jsx("marker", { id: "snippy-arrowhead", markerWidth: "10", markerHeight: "10", refX: "8", refY: "3", orient: "auto", children: _jsx("path", { d: "M0,0 L0,6 L9,3 z", fill: "#3DDBB0" }) }) }), annotations.map((ann, idx) => {
                    const stroke = ann.color ?? '#3DDBB0';
                    if (ann.kind === 'circle') {
                        const r = Math.max(18, (ann.width ?? 60) / 2);
                        return (_jsxs("g", { children: [_jsx("circle", { cx: ann.x, cy: ann.y, r: r, fill: "rgba(61, 219, 176, 0.1)", stroke: stroke, strokeWidth: "3" }), ann.text ? (_jsx("text", { x: ann.x, y: ann.y - r - 8, fill: "#F3F4F7", fontSize: "12", fontWeight: "600", fontFamily: "Inter, system-ui, sans-serif", children: ann.text })) : null] }, idx));
                    }
                    if (ann.kind === 'arrow' && ann.x2 != null && ann.y2 != null) {
                        const cx = Math.round((ann.x + ann.x2) / 2 - 40);
                        const cy = Math.round((ann.y + ann.y2) / 2 + 20);
                        return (_jsxs("g", { children: [_jsx("path", { d: `M ${ann.x} ${ann.y} Q ${cx} ${cy} ${ann.x2} ${ann.y2}`, fill: "none", stroke: stroke, strokeWidth: "3", strokeLinecap: "round" }), _jsx("circle", { cx: ann.x2, cy: ann.y2, r: "5", fill: stroke }), ann.text ? (_jsx("text", { x: ann.x, y: ann.y - 8, fill: "#F3F4F7", fontSize: "12", fontWeight: "600", fontFamily: "Inter, system-ui, sans-serif", children: ann.text })) : null] }, idx));
                    }
                    if (ann.kind === 'label') {
                        return (_jsxs("g", { transform: `translate(${ann.x}, ${ann.y})`, children: [_jsx("rect", { x: "0", y: "0", width: Math.max(90, (ann.text?.length ?? 6) * 8 + 20), height: "28", rx: "8", fill: "#14151A", stroke: stroke, strokeWidth: "1.5" }), _jsx("text", { x: "10", y: "18", fill: "#F3F4F7", fontSize: "12", fontWeight: "600", fontFamily: "Inter, system-ui, sans-serif", children: ann.text ?? 'Snippy' })] }, idx));
                    }
                    const w = Math.max(40, ann.width ?? 160);
                    const h = Math.max(28, ann.height ?? 44);
                    return (_jsxs("g", { children: [_jsx("rect", { x: ann.x, y: ann.y, width: w, height: h, rx: "8", fill: "rgba(15, 163, 127, 0.08)", stroke: stroke, strokeWidth: "2.5", strokeDasharray: "6 4" }), ann.text ? (_jsx("text", { x: ann.x + 6, y: Math.max(16, ann.y - 6), fill: "#F3F4F7", fontSize: "12", fontWeight: "600", fontFamily: "Inter, system-ui, sans-serif", children: ann.text })) : null] }, idx));
                })] }));
    }
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
            }, children: ["#", payload?.step_number ?? 1, " ", payload?.label ?? 'Snippy target'] }) }));
}

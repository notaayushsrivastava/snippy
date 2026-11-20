import { useEffect, useState } from 'react';
import { fetchLatestWindowsRelease, RELEASES_PAGE_URL, type WindowsReleaseInfo } from './lib/releases.js';

const STEPS = [
  {
    n: '1',
    bg: '#DDF5EC',
    fg: '#065A44',
    title: 'Summon it',
    body: 'Press one shortcut anywhere. A search-style bar drops in from the top of your screen. Type a task and choose the window Snippy may look at.',
  },
  {
    n: '2',
    bg: '#E4E9FF',
    fg: '#1F4FD1',
    title: 'Watch it work',
    body: 'Snippy circles the control it is about to use and explains the step in plain words. Pause, steer or take over whenever you like.',
  },
  {
    n: '3',
    bg: '#FFEBC7',
    fg: '#7A4E00',
    title: 'Approve what matters',
    body: 'Saving, sending, submitting and deleting wait for your yes. Passwords and codes are always yours to type.',
  },
];

const RULES = [
  {
    name: 'Observe',
    eg: 'Read the selected window and inspect its controls.',
    rule: 'Runs on its own, in scope',
    bg: 'rgba(61,219,176,0.16)',
    fg: '#7BE8C6',
  },
  {
    name: 'Navigate',
    eg: 'Focus, scroll, open menus, switch a selected tab.',
    rule: 'Runs on its own, in scope',
    bg: 'rgba(61,219,176,0.16)',
    fg: '#7BE8C6',
  },
  {
    name: 'Draft',
    eg: 'Type non-sensitive text into a draft field or editor buffer.',
    rule: 'Shown first, then runs if allowed',
    bg: 'rgba(141,184,255,0.16)',
    fg: '#B5D0FF',
  },
  {
    name: 'Change local state',
    eg: 'Save or overwrite files, install software, change settings.',
    rule: 'Asks you first',
    bg: 'rgba(255,184,77,0.16)',
    fg: '#FFC76E',
  },
  {
    name: 'External impact',
    eg: 'Send, post, submit, purchase, delete, upload or download.',
    rule: 'Asks at every step',
    bg: 'rgba(255,184,77,0.16)',
    fg: '#FFC76E',
  },
  {
    name: 'Sensitive input',
    eg: 'Passwords, card numbers, one-time and recovery codes.',
    rule: 'You take over',
    bg: 'rgba(184,164,255,0.18)',
    fg: '#CBBEFF',
  },
];

const PRIVACY_ITEMS = [
  {
    title: 'Local first',
    body: 'Task history, screenshots and action records are stored encrypted on your device. You can export or delete any session.',
  },
  {
    title: 'Keys in your OS keychain',
    body: 'Your API key lives in Credential Manager on Windows. It never appears in logs or analytics.',
  },
  {
    title: 'Only what it needs',
    body: 'Snippy sends the current window context to the provider you chose, not your whole desktop.',
  },
  {
    title: 'Pages cannot give it orders',
    body: 'Text on web pages, in documents and in code is treated as data, never as instructions.',
  },
  {
    title: 'Secrets stay secret',
    body: 'Sensitive fields are never observed, stored or typed by Snippy.',
  },
];

const FLOWS = [
  {
    tag: 'Browser + sheet',
    title: 'Research into a table',
    body: 'Gather public facts and fill a draft table. Snippy stops before anything is submitted or shared.',
  },
  {
    tag: 'Word and Excel',
    title: 'Documents and spreadsheets',
    body: 'Find, format and draft content. It asks before saving or overwriting a file.',
  },
  {
    tag: 'VS Code extension',
    title: 'Focused code changes',
    body: 'Explain what is on screen, draft an edit and show the diff. Every file write needs approval.',
  },
  {
    tag: 'Any visible app',
    title: 'Everyday navigation',
    body: 'Open, locate and inspect content across supported apps using their on-screen controls.',
  },
];

const LIMITS = [
  {
    title: 'Passwords, payments and codes',
    body: 'It never reads, stores or enters them. You take over for those fields.',
  },
  {
    title: 'Admin prompts and secure screens',
    body: 'It does not control elevated apps, UAC prompts or the secure desktop.',
  },
  {
    title: 'Unattended background jobs',
    body: 'Snippy works while you are present and can be stopped at any moment.',
  },
  {
    title: 'Signing in to ChatGPT or Gemini accounts',
    body: 'It connects with an API key you supply, not through consumer logins.',
  },
  {
    title: 'macOS, Linux, mobile and team features',
    body: 'This public beta supports Windows 11. macOS support is coming soon.',
  },
];

const PREVIEW_STATES = [
  {
    step: 'Step 3 of 6 · Chrome to Excel',
    title: 'Copying the Flat white price for café 6',
    next: 'Read the price from the table, then draft it into B7',
    reason: 'I can read this cell directly, so I do not need to guess from pixels.',
    progress: '50%',
    highlight: 'Flat white · café 6',
    approval: 'Overwrite cafe-prices.xlsx?',
  },
  {
    step: 'Step 6 of 6 · Excel',
    title: 'Waiting for approval to save workbook',
    next: 'Save cafe-prices.xlsx with 6 updated rows',
    reason: 'Saving modifies your local file on disk, so Snippy pauses and asks first.',
    progress: '92%',
    highlight: 'Save button · Ribbon',
    approval: 'Save cafe-prices.xlsx?',
  },
];

function WindowsIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M1 2.5 7.5 1.6v5.9H1V2.5Zm7.5-1L15 .6v6.9H8.5V1.5ZM1 8.5h6.5v5.9L1 13.5V8.5Zm7.5 0H15v6.9l-6.5-.9V8.5Z" />
    </svg>
  );
}

function AppleIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M11.6 8.4c0-1.9 1.5-2.8 1.6-2.9-.9-1.3-2.2-1.5-2.7-1.5-1.1-.1-2.2.7-2.8.7-.6 0-1.5-.7-2.4-.7-1.2 0-2.4.7-3 1.9-1.3 2.2-.3 5.5.9 7.3.6.9 1.3 1.9 2.3 1.8.9 0 1.3-.6 2.4-.6s1.4.6 2.4.6c1 0 1.6-.9 2.2-1.8.7-1 1-2 1-2.1-.1 0-1.9-.7-1.9-2.7ZM9.7 2.8c.5-.6.9-1.5.8-2.3-.7 0-1.6.5-2.1 1.1-.5.5-.9 1.4-.8 2.2.8.1 1.6-.4 2.1-1Z" />
    </svg>
  );
}

export function App() {
  const [release, setRelease] = useState<WindowsReleaseInfo>({
    version: null,
    windowsDownloadUrl: RELEASES_PAGE_URL,
    assetName: null,
  });
  const [previewIdx, setPreviewIdx] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    void fetchLatestWindowsRelease(controller.signal).then(setRelease);
    return () => controller.abort();
  }, []);

  const preview = PREVIEW_STATES[previewIdx];

  return (
    <div className="web-container">
      <header className="web-header">
        <a href="#top" aria-label="Snippy home" className="web-brand">
          <svg
            width="30"
            height="30"
            viewBox="0 0 28 28"
            fill="none"
            stroke="#087A5B"
            strokeWidth="2.4"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M4 10V6a2 2 0 0 1 2-2h4M18 4h4a2 2 0 0 1 2 2v4M24 18v4a2 2 0 0 1-2 2h-4M10 24H6a2 2 0 0 1-2-2v-4" />
            <circle cx="14" cy="14" r="3" fill="#087A5B" stroke="none" />
          </svg>
          Snippy
        </a>
        <nav aria-label="Primary" className="web-nav">
          <a href="#how" className="web-nav-link">
            How it works
          </a>
          <a href="#control" className="web-nav-link">
            Control
          </a>
          <a href="#privacy" className="web-nav-link">
            Privacy
          </a>
          <a href="#faq" className="web-nav-link">
            Limits
          </a>
        </nav>
        <a href="#download" className="web-btn-primary-sm">
          Get the beta
        </a>
      </header>

      <main>
        <section id="top" className="web-hero">
          <div className="web-beta-pill">
            <span className="web-beta-dot" aria-hidden="true" />
            Public beta{release.version ? ` ${release.version}` : ''} · Windows 11 (macOS coming soon)
          </div>
          <h1 className="web-hero-title">Tell it what you need done. Say yes before it matters.</h1>
          <p className="web-hero-sub">
            Snippy is an assistant that sees the window you choose, draws on your screen to show what it will do next,
            and works your apps for you. It stops and asks before anything consequential.
          </p>

          <div id="download" className="web-cta-row">
            <a
              href={release.windowsDownloadUrl}
              className="web-btn-win"
              rel="noopener noreferrer"
              title={release.assetName ? `Download ${release.assetName}` : 'Download latest Windows installer'}
            >
              <WindowsIcon />
              Download for Windows
            </a>
            <button
              type="button"
              disabled
              aria-disabled="true"
              className="web-btn-mac-disabled"
              title="macOS build is coming soon"
            >
              <AppleIcon />
              Download for macOS
              <span className="web-soon-tag">Coming soon</span>
            </button>
          </div>
          <div className="web-hero-note">
            Bring your own OpenAI or Gemini key. No Snippy account. Your keys and history stay on your computer.
          </div>
        </section>

        <section aria-label="Interactive product preview" className="web-showcase">
          <div className="web-cmd-bar">
            <div
              style={{
                display: 'flex',
                flexWrap: 'wrap',
                alignItems: 'center',
                gap: '14px',
                padding: '14px 16px 14px 20px',
              }}
            >
              <div className="web-pulse-dot" aria-hidden="true">
                <div className="web-pulse-inner" />
              </div>
              <div style={{ flex: '1 1 200px', minWidth: 0 }}>
                <div style={{ fontSize: '15px', fontWeight: 600, lineHeight: '20px' }}>{preview.title}</div>
                <div style={{ fontSize: '12.5px', color: '#8B90A0', lineHeight: '17px' }}>{preview.step}</div>
              </div>
              <button
                type="button"
                onClick={() => setPreviewIdx((i) => (i === 0 ? 1 : 0))}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  minHeight: '34px',
                  padding: '0 14px',
                  borderRadius: '10px',
                  border: '1px solid rgba(255,255,255,0.2)',
                  background: 'transparent',
                  color: '#F3F4F7',
                  fontSize: '13px',
                  fontWeight: 500,
                  cursor: 'pointer',
                  fontFamily: 'inherit',
                }}
              >
                {previewIdx === 0 ? 'Preview approval' : 'Preview step'}
              </button>
              <span
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  minHeight: '34px',
                  padding: '0 14px',
                  borderRadius: '10px',
                  background: '#FF7A7A',
                  color: '#1A0B0B',
                  fontSize: '13px',
                  fontWeight: 600,
                }}
              >
                Stop
              </span>
            </div>
            <div style={{ height: '3px', background: 'rgba(255,255,255,0.08)' }}>
              <div
                style={{
                  width: preview.progress,
                  height: '3px',
                  background: '#3DDBB0',
                  transition: 'width 240ms ease',
                }}
              />
            </div>
            <div style={{ padding: '16px 20px' }}>
              <div
                style={{
                  fontSize: '11px',
                  fontWeight: 600,
                  letterSpacing: '0.08em',
                  textTransform: 'uppercase',
                  color: '#8B90A0',
                }}
              >
                Next step
              </div>
              <div style={{ marginTop: '6px', fontSize: '16px', fontWeight: 600, lineHeight: '22px' }}>
                {preview.next}
              </div>
              <div style={{ marginTop: '4px', fontSize: '13px', lineHeight: '19px', color: '#A7ACB8' }}>
                {preview.reason}
              </div>
            </div>
          </div>

          <div
            style={{
              display: 'flex',
              flexWrap: 'wrap',
              gap: '16px',
              maxWidth: '760px',
              margin: '20px auto 0',
            }}
          >
            <div
              style={{
                flex: '1 1 260px',
                border: '2px solid #0B8F6C',
                borderRadius: '14px',
                background: 'rgba(15,163,127,0.1)',
                padding: '16px',
              }}
            >
              <div style={{ fontSize: '12px', fontWeight: 600, color: '#065A44' }}>Circled on your screen</div>
              <div style={{ marginTop: '6px', fontSize: '14px', lineHeight: '20px', color: '#1B1E27' }}>
                {preview.highlight}
              </div>
            </div>
            <div
              style={{
                flex: '1 1 260px',
                background: '#14151A',
                border: '1px solid rgba(255,184,77,0.55)',
                borderRadius: '14px',
                padding: '16px',
                color: '#F3F4F7',
              }}
            >
              <div
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  minHeight: '22px',
                  padding: '0 9px',
                  borderRadius: '999px',
                  background: 'rgba(255,184,77,0.16)',
                  color: '#FFC76E',
                  fontSize: '11.5px',
                  fontWeight: 700,
                }}
              >
                Needs approval
              </div>
              <div style={{ marginTop: '8px', fontSize: '15px', fontWeight: 600, lineHeight: '20px' }}>
                {preview.approval}
              </div>
            </div>
          </div>
        </section>

        <section id="how" style={{ padding: '104px 0 24px' }}>
          <div className="web-section-eyebrow">How it works</div>
          <h2 className="web-section-title">One shortcut. A search bar. A helper you can watch.</h2>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '20px', marginTop: '36px' }}>
            {STEPS.map((s) => (
              <div
                key={s.n}
                style={{
                  flex: '1 1 300px',
                  background: '#FFFFFF',
                  border: '1px solid #DDE0E7',
                  borderRadius: '20px',
                  padding: '28px',
                }}
              >
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    width: '40px',
                    height: '40px',
                    borderRadius: '12px',
                    background: s.bg,
                    color: s.fg,
                    fontSize: '16px',
                    fontWeight: 700,
                  }}
                >
                  {s.n}
                </div>
                <h3
                  style={{
                    margin: '18px 0 0',
                    fontSize: '21px',
                    lineHeight: 1.25,
                    fontWeight: 700,
                    letterSpacing: '-0.02em',
                  }}
                >
                  {s.title}
                </h3>
                <p style={{ margin: '8px 0 0', fontSize: '15px', lineHeight: 1.6, color: '#4A5060' }}>{s.body}</p>
              </div>
            ))}
          </div>
        </section>

        <section id="control" className="web-dark-panel">
          <div className="web-section-eyebrow" style={{ color: '#3DDBB0' }}>
            Control
          </div>
          <h2 className="web-section-title" style={{ maxWidth: '760px' }}>
            It stops for you before anything consequential.
          </h2>
          <p style={{ margin: '14px 0 0', maxWidth: '640px', fontSize: '16px', lineHeight: 1.6, color: '#A7ACB8' }}>
            Every action belongs to a category. The category decides whether Snippy goes ahead, asks, or hands control
            to you.
          </p>
          <div style={{ marginTop: '32px', borderTop: '1px solid rgba(255,255,255,0.1)' }}>
            {RULES.map((r) => (
              <div
                key={r.name}
                style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  alignItems: 'center',
                  gap: '12px 24px',
                  padding: '18px 0',
                  borderBottom: '1px solid rgba(255,255,255,0.1)',
                }}
              >
                <div style={{ flex: '1 1 180px', fontSize: '16px', fontWeight: 600 }}>{r.name}</div>
                <div style={{ flex: '2 1 280px', fontSize: '14.5px', lineHeight: 1.5, color: '#A7ACB8' }}>{r.eg}</div>
                <span
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    minHeight: '28px',
                    padding: '0 12px',
                    borderRadius: '999px',
                    background: r.bg,
                    color: r.fg,
                    fontSize: '12.5px',
                    fontWeight: 600,
                  }}
                >
                  {r.rule}
                </span>
              </div>
            ))}
          </div>
          <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '12px', marginTop: '24px' }}>
            <span
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                minHeight: '34px',
                padding: '0 14px',
                borderRadius: '10px',
                background: '#FF7A7A',
                color: '#1A0B0B',
                fontSize: '14px',
                fontWeight: 700,
              }}
            >
              Stop
            </span>
            <span style={{ fontSize: '14px', lineHeight: 1.5, color: '#A7ACB8' }}>
              Always on screen, with a global shortcut. It cancels everything queued.
            </span>
          </div>
        </section>

        <section
          id="privacy"
          style={{ display: 'flex', flexWrap: 'wrap', gap: '40px', marginTop: '104px', alignItems: 'flex-start' }}
        >
          <div style={{ flex: '1 1 360px' }}>
            <div className="web-section-eyebrow">Privacy</div>
            <h2 className="web-section-title">Your computer, your keys, your history.</h2>
            <p style={{ margin: '16px 0 0', fontSize: '16px', lineHeight: 1.65, color: '#4A5060' }}>
              Snippy runs on your machine and talks straight to the model provider you pick. There is no Snippy cloud in
              the middle.
            </p>
          </div>
          <div style={{ flex: '1 1 440px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
            {PRIVACY_ITEMS.map((p) => (
              <div
                key={p.title}
                style={{
                  display: 'flex',
                  gap: '16px',
                  alignItems: 'flex-start',
                  background: '#FFFFFF',
                  border: '1px solid #DDE0E7',
                  borderRadius: '16px',
                  padding: '20px',
                }}
              >
                <div
                  style={{
                    flex: 'none',
                    width: '32px',
                    height: '32px',
                    borderRadius: '10px',
                    background: '#DDF5EC',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                  }}
                >
                  <svg
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="#065A44"
                    strokeWidth="2.6"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    aria-hidden="true"
                  >
                    <path d="M5 12.5l4.5 4.5L19 7" />
                  </svg>
                </div>
                <div>
                  <div style={{ fontSize: '16px', fontWeight: 600, lineHeight: '22px' }}>{p.title}</div>
                  <div style={{ marginTop: '4px', fontSize: '14px', lineHeight: 1.55, color: '#4A5060' }}>{p.body}</div>
                </div>
              </div>
            ))}
          </div>
        </section>

        <section style={{ marginTop: '104px' }}>
          <div className="web-section-eyebrow">Works with</div>
          <h2 className="web-section-title">The apps you already use, through their visible controls.</h2>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '20px', marginTop: '36px' }}>
            {FLOWS.map((f) => (
              <div
                key={f.title}
                style={{
                  flex: '1 1 260px',
                  background: '#FFFFFF',
                  border: '1px solid #DDE0E7',
                  borderRadius: '20px',
                  padding: '26px',
                }}
              >
                <div
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    minHeight: '26px',
                    padding: '0 10px',
                    borderRadius: '999px',
                    background: '#EEF0F5',
                    color: '#2E3340',
                    fontSize: '12px',
                    fontWeight: 600,
                  }}
                >
                  {f.tag}
                </div>
                <h3
                  style={{
                    margin: '14px 0 0',
                    fontSize: '19px',
                    lineHeight: 1.25,
                    fontWeight: 700,
                    letterSpacing: '-0.02em',
                  }}
                >
                  {f.title}
                </h3>
                <p style={{ margin: '8px 0 0', fontSize: '14.5px', lineHeight: 1.6, color: '#4A5060' }}>{f.body}</p>
              </div>
            ))}
          </div>
        </section>

        <section
          id="faq"
          style={{ display: 'flex', flexWrap: 'wrap', gap: '40px', marginTop: '104px', alignItems: 'flex-start' }}
        >
          <div style={{ flex: '1 1 320px' }}>
            <div className="web-section-eyebrow">Limits</div>
            <h2 className="web-section-title">What Snippy will not do.</h2>
            <p style={{ margin: '16px 0 0', fontSize: '16px', lineHeight: 1.65, color: '#4A5060' }}>
              Clear edges are part of the promise. In this beta, Snippy deliberately stays out of these areas.
            </p>
          </div>
          <div style={{ flex: '1 1 460px', borderTop: '1px solid #DDE0E7' }}>
            {LIMITS.map((l) => (
              <div key={l.title} style={{ padding: '20px 0', borderBottom: '1px solid #DDE0E7' }}>
                <div style={{ fontSize: '16px', fontWeight: 600, lineHeight: '22px' }}>{l.title}</div>
                <div style={{ marginTop: '4px', fontSize: '14.5px', lineHeight: 1.6, color: '#4A5060' }}>{l.body}</div>
              </div>
            ))}
          </div>
        </section>

        <section
          style={{
            margin: '104px 0 0',
            borderRadius: '28px',
            background: '#14151A',
            color: '#F3F4F7',
            padding: 'clamp(32px,6vw,72px)',
            textAlign: 'center',
          }}
        >
          <h2
            style={{
              margin: '0 auto',
              maxWidth: '720px',
              fontSize: 'clamp(30px,4.4vw,48px)',
              lineHeight: 1.08,
              fontWeight: 700,
              letterSpacing: '-0.03em',
            }}
          >
            Hand off the busywork. Keep the final say.
          </h2>
          <p style={{ margin: '16px auto 0', maxWidth: '520px', fontSize: '16px', lineHeight: 1.6, color: '#A7ACB8' }}>
            Install the beta, add your own key, and pick a window.
          </p>
          <div className="web-cta-row" style={{ marginTop: '28px' }}>
            <a
              href={release.windowsDownloadUrl}
              className="web-btn-win web-btn-win-bright"
              rel="noopener noreferrer"
              title={release.assetName ? `Download ${release.assetName}` : 'Download latest Windows installer'}
            >
              <WindowsIcon />
              Download for Windows
            </a>
            <button
              type="button"
              disabled
              aria-disabled="true"
              className="web-btn-mac-disabled web-btn-mac-disabled-dark"
              title="macOS build is coming soon"
            >
              <AppleIcon />
              Download for macOS
              <span className="web-soon-tag web-soon-tag-dark">Coming soon</span>
            </button>
          </div>
        </section>
      </main>

      <footer className="web-footer">
        <div style={{ fontWeight: 700, color: '#14151A' }}>Snippy</div>
        <a href="#privacy">Privacy</a>
        <a href="#faq">Limits</a>
        <a href="#control">How approvals work</a>
        <a href={RELEASES_PAGE_URL} rel="noopener noreferrer">
          Releases
        </a>
        <div style={{ flex: 1 }} />
        <div>
          Snippy · Beta build {release.version ?? '0.1.0'}
        </div>
      </footer>
    </div>
  );
}

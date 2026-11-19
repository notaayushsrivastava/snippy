import styles from './ui.module.css';

export interface SecretHandoffProps {
  reason: string;
  onContinue: () => void;
  onStop: () => void;
}

export function SecretHandoff({ reason, onContinue, onStop }: SecretHandoffProps) {
  return (
    <section
      className={styles.secretHandoff}
      role="alertdialog"
      aria-labelledby="secret-handoff-title"
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
        <svg
          width="22"
          height="22"
          viewBox="0 0 24 24"
          fill="none"
          stroke="var(--status-secret)"
          strokeWidth="2.2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M3 12s3.5-6 9-6 9 6 9 6-3.5 6-9 6-9-6-9-6z" />
          <circle cx="12" cy="12" r="2.6" />
          <path d="M4 20L20 4" />
        </svg>
        <div>
          <div id="secret-handoff-title" style={{ fontSize: '15px', fontWeight: 600 }}>
            Your turn: enter sensitive details yourself
          </div>
          <p className={styles.muted}>Snippy stopped looking at this window and is waiting.</p>
        </div>
      </div>
      <p className={styles.muted}>{reason}</p>
      <div className={styles.buttonRow}>
        <button
          type="button"
          className={`${styles.btnViolet} ${styles.focusable}`}
          onClick={onContinue}
        >
          I am done, resume
        </button>
        <button
          type="button"
          className={`${styles.stopButton} ${styles.focusable}`}
          onClick={onStop}
        >
          Stop
        </button>
      </div>
    </section>
  );
}

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
      <strong id="secret-handoff-title">
        Snippy paused. Enter this yourself, then continue.
      </strong>
      <p className={styles.muted}>{reason}</p>
      <div className={styles.buttonRow}>
        <button
          type="button"
          className={`${styles.btnPrimary} ${styles.focusable}`}
          onClick={onContinue}
        >
          Continue
        </button>
        <button
          type="button"
          className={`${styles.btnDanger} ${styles.focusable}`}
          onClick={onStop}
        >
          Stop
        </button>
      </div>
    </section>
  );
}

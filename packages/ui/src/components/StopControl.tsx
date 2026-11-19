import styles from './ui.module.css';

export interface StopControlProps {
  onStop: () => void;
  shortcutLabel?: string;
}

export function StopControl({ onStop, shortcutLabel = 'Esc' }: StopControlProps) {
  return (
    <button
      type="button"
      className={`${styles.stopButton} ${styles.focusable}`}
      onClick={onStop}
      aria-label={`Stop task (${shortcutLabel})`}
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <rect x="4" y="4" width="16" height="16" rx="3" />
      </svg>
      <span>Stop</span>
      <kbd className={styles.stopKbd}>{shortcutLabel}</kbd>
    </button>
  );
}

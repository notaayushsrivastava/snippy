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
      <span>Stop</span>
      <kbd className={styles.badge}>{shortcutLabel}</kbd>
    </button>
  );
}

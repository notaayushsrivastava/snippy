import type { Scope } from '@snippy/protocol';
import styles from './ui.module.css';

export interface ScopeChipProps {
  scope: Scope;
  onChangeScope?: () => void;
}

export function ScopeChip({ scope, onChangeScope }: ScopeChipProps) {
  return (
    <div className={styles.scopeChip} role="status" aria-live="polite">
      <span>
        Seeing: <strong>{scope.app_name}</strong> — {scope.window_title}
      </span>
      {onChangeScope ? (
        <button
          type="button"
          className={`${styles.btnSecondary} ${styles.focusable}`}
          onClick={onChangeScope}
        >
          Change
        </button>
      ) : null}
    </div>
  );
}

import type { Scope } from '@snippy/protocol';
import styles from './ui.module.css';

export interface ScopeChipProps {
  scope: Scope;
  onChangeScope?: () => void;
}

export function ScopeChip({ scope, onChangeScope }: ScopeChipProps) {
  return (
    <div className={styles.scopeChip} role="status" aria-live="polite">
      <span className={styles.scopeDot} aria-hidden="true" />
      <span>
        {scope.app_name} · {scope.window_title}
      </span>
      {onChangeScope ? (
        <button
          type="button"
          className={`${styles.btnGhost} ${styles.focusable}`}
          style={{ height: '22px', padding: '0 6px', fontSize: '11.5px' }}
          onClick={onChangeScope}
        >
          Change
        </button>
      ) : null}
    </div>
  );
}

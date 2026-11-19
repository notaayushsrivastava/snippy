import type { ApprovedAction } from '@snippy/protocol';
import styles from './ui.module.css';

export interface ApprovalCardProps {
  action: ApprovedAction;
  consequence: string;
  onApprove: () => void;
  onTakeOver: () => void;
  onReject: () => void;
}

export function ApprovalCard({
  action,
  consequence,
  onApprove,
  onTakeOver,
  onReject,
}: ApprovalCardProps) {
  const isDestructive = action.risk === 'external_impact';
  const targetLabel = action.target.semantic_locator ?? 'Selected target';

  return (
    <section
      className={styles.approvalCard}
      role="alertdialog"
      aria-labelledby="approval-title"
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
        <span className={`${styles.pill} ${styles.pillAmber}`}>Needs approval</span>
        <span className={styles.muted}>{action.risk}</span>
      </div>
      <div id="approval-title" style={{ fontSize: '18px', fontWeight: 600, lineHeight: '24px' }}>
        {action.kind} on {targetLabel}?
      </div>
      <p className={styles.muted}>{consequence} — {action.reason}</p>
      <div className={styles.buttonRow}>
        {isDestructive ? (
          <>
            <button
              type="button"
              className={`${styles.btnSecondary} ${styles.focusable}`}
              onClick={onReject}
            >
              Reject
            </button>
            <button
              type="button"
              className={`${styles.btnSecondary} ${styles.focusable}`}
              onClick={onTakeOver}
            >
              Take over
            </button>
            <button
              type="button"
              className={`${styles.btnApprove} ${styles.focusable}`}
              onClick={onApprove}
            >
              <span>Approve</span>
              <span className={styles.stopKbd}>⏎</span>
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className={`${styles.btnApprove} ${styles.focusable}`}
              onClick={onApprove}
            >
              <span>Approve</span>
              <span className={styles.stopKbd}>⏎</span>
            </button>
            <button
              type="button"
              className={`${styles.btnSecondary} ${styles.focusable}`}
              onClick={onTakeOver}
            >
              Take over
            </button>
            <button
              type="button"
              className={`${styles.btnGhost} ${styles.focusable}`}
              onClick={onReject}
            >
              Reject
            </button>
          </>
        )}
      </div>
    </section>
  );
}

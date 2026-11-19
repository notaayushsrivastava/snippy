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
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <strong id="approval-title">Needs your approval</strong>
        <span className={styles.badge}>{action.risk}</span>
      </div>
      <p>
        Action: <strong>{action.kind}</strong> on <strong>{targetLabel}</strong>
      </p>
      <p className={styles.muted}>Consequence: {consequence}</p>
      <p className={styles.muted}>Reason: {action.reason}</p>
      <div className={styles.buttonRow}>
        {isDestructive ? (
          <>
            <button
              type="button"
              className={`${styles.btnDanger} ${styles.focusable}`}
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
              className={`${styles.btnPrimary} ${styles.focusable}`}
              onClick={onApprove}
            >
              Approve
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className={`${styles.btnPrimary} ${styles.focusable}`}
              onClick={onApprove}
            >
              Approve
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
              className={`${styles.btnDanger} ${styles.focusable}`}
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

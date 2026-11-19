import type { TimelineStep } from '@snippy/protocol';
import styles from './ui.module.css';

export interface TimelineItemProps {
  step: TimelineStep;
}

export function TimelineItem({ step }: TimelineItemProps) {
  const statusLabel = step.verification?.status ?? 'pending';

  return (
    <li className={styles.card}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span className={styles.tabular}>
          #{step.step_number} — {step.action_summary}
        </span>
        <span className={styles.badge}>{statusLabel}</span>
      </div>
      <details>
        <summary className={styles.muted}>Technical details</summary>
        <p className={styles.muted}>Target: {step.target_label}</p>
        <p className={styles.muted}>Policy: {step.policy_decision}</p>
        <p className={styles.muted}>Observation hash: {step.observation_hash}</p>
      </details>
    </li>
  );
}

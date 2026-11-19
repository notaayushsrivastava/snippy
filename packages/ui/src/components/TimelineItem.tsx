import type { TimelineStep } from '@snippy/protocol';
import styles from './ui.module.css';

export interface TimelineItemProps {
  step: TimelineStep;
}

export function TimelineItem({ step }: TimelineItemProps) {
  const statusLabel = step.verification?.status ?? 'pending';

  return (
    <li className={styles.timelineRow}>
      <div className={styles.timelineMain}>
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="var(--status-safe)"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="9" />
          <path d="M8 12.5l3 3 5-6" />
        </svg>
        <span className={styles.tabular} style={{ flex: 1, fontSize: '13.5px' }}>
          {step.action_summary}
        </span>
        <span className={styles.pill}>{statusLabel}</span>
      </div>
      <details style={{ paddingLeft: '28px' }}>
        <summary className={styles.muted} style={{ cursor: 'pointer' }}>
          Technical details
        </summary>
        <p className={styles.muted}>
          Target: {step.target_label} · Policy: {step.policy_decision} · Hash:{' '}
          {step.observation_hash.slice(0, 12)}
        </p>
      </details>
    </li>
  );
}

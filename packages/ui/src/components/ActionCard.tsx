import type { RiskCategory } from '@snippy/protocol';
import styles from './ui.module.css';

export interface ActionCardProps {
  stepNumber: number;
  verb: string;
  target: string;
  reason: string;
  expectedResult: string;
  risk: RiskCategory;
}

export function ActionCard({
  stepNumber,
  verb,
  target,
  reason,
  expectedResult,
  risk,
}: ActionCardProps) {
  return (
    <article className={styles.card} aria-label={`Step ${stepNumber}: ${verb}`}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <strong className={styles.tabular}>
          Step {stepNumber}: {verb} {target}
        </strong>
        <span className={styles.badge}>{risk}</span>
      </div>
      <p className={styles.muted}>{reason}</p>
      <p className={styles.muted}>Expected: {expectedResult}</p>
    </article>
  );
}

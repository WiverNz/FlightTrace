import type {
  AssertionOutcome,
  FailureReason,
  MissionStatus,
} from '../types/api';

interface StatusProps {
  status: MissionStatus;
  failureReason?: FailureReason | null;
}

export function StatusBadge({ status, failureReason }: StatusProps) {
  const tone = toneForStatus(status);
  const label =
    status === 'FAILED' && failureReason
      ? `${status} — ${failureReason}`
      : status;
  return <span className={`badge ${tone}`}>{label}</span>;
}

interface OutcomeProps {
  outcome: AssertionOutcome;
}

export function OutcomeBadge({ outcome }: OutcomeProps) {
  return <span className={`badge ${outcome.toLowerCase()}`}>{outcome}</span>;
}

function toneForStatus(status: MissionStatus): string {
  switch (status) {
    case 'SUCCEEDED':
      return 'pass';
    case 'FAILED':
      return 'fail';
    case 'RUNNING':
      return 'accent';
  }
}

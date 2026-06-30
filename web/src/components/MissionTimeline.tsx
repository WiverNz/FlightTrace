import type { PhaseTransitionDto } from '../types/api';
import { formatIso, formatRelativeTime } from '../format';

interface Props {
  phases: PhaseTransitionDto[];
  startedAt: string;
}

export function MissionTimeline({ phases, startedAt }: Props) {
  if (phases.length === 0) {
    return <p className="muted">No phase transitions recorded.</p>;
  }

  return (
    <ol className="timeline">
      {phases.map((p, i) => (
        <li key={i} className={p.phase === 'FAILED' ? 'failed' : ''}>
          <span className="at">
            <strong>{formatRelativeTime(p.at, startedAt)}</strong>
            <span>{formatIso(p.at)}</span>
          </span>
          <span className="dot" aria-hidden="true" />
          <span className="phase">{p.phase}</span>
        </li>
      ))}
    </ol>
  );
}

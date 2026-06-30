import type { PhaseTransitionDto } from '../types/api';
import { formatIso } from '../format';

interface Props {
  phases: PhaseTransitionDto[];
}

export function MissionTimeline({ phases }: Props) {
  if (phases.length === 0) {
    return <p className="muted">No phase transitions recorded.</p>;
  }

  return (
    <ol className="timeline">
      {phases.map((p, i) => (
        <li key={i} className={p.phase === 'FAILED' ? 'failed' : ''}>
          <span className="at">{formatIso(p.at)}</span>
          <span className="dot" aria-hidden="true" />
          <span className="phase">{p.phase}</span>
        </li>
      ))}
    </ol>
  );
}

import type { EvidenceDto } from '../types/api';
import { formatTimeOfDay, shortId } from '../format';

interface Props {
  evidence: EvidenceDto[];
}

export function EvidenceList({ evidence }: Props) {
  if (evidence.length === 0) {
    return <p className="muted">No evidence recorded.</p>;
  }

  return (
    <ul className="evidence-list">
      {evidence.map((item, index) => (
        <li key={index}>
          <span className="kind">{item.kind}</span>
          <span className="body">{renderBody(item)}</span>
        </li>
      ))}
    </ul>
  );
}

function renderBody(item: EvidenceDto): React.ReactNode {
  switch (item.kind) {
    case 'Note':
      return item.text;
    case 'Event':
      return (
        <>
          event <code className="mono">{shortId(item.eventId)}</code>
        </>
      );
    case 'Sample':
      return (
        <>
          <code className="mono">{formatTimeOfDay(item.at)}</code> ·{' '}
          <strong>{item.field}</strong> ={' '}
          <code className="mono">{item.value}</code>
        </>
      );
    case 'Statustext':
      return (
        <>
          <code className="mono">{formatTimeOfDay(item.at)}</code> · “{item.text}”
        </>
      );
  }
}

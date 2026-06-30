import type { AssertionResultDto } from '../types/api';
import { OutcomeBadge } from './StatusBadge';
import { formatIso } from '../format';

interface Props {
  assertions: AssertionResultDto[];
}

const OUTCOME_RANK: Record<AssertionResultDto['outcome'], number> = {
  FAIL: 0,
  WARN: 1,
  PASS: 2,
};

export function AssertionTable({ assertions }: Props) {
  if (assertions.length === 0) {
    return <p className="muted">No assertions evaluated for this run.</p>;
  }

  const sorted = [...assertions].sort(
    (a, b) => OUTCOME_RANK[a.outcome] - OUTCOME_RANK[b.outcome],
  );

  return (
    <div className="assertion-table-wrap">
      <table className="data">
        <thead>
          <tr>
            <th>Name</th>
            <th>Outcome</th>
            <th>Detail</th>
            <th>Evaluated</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((a) => (
            <tr key={a.id}>
              <td className="mono">{a.name}</td>
              <td>
                <OutcomeBadge outcome={a.outcome} />
              </td>
              <td>{a.detail || <span className="muted">—</span>}</td>
              <td className="mono muted">{formatIso(a.evaluatedAt)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

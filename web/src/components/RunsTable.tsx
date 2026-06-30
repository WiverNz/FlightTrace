import { Link } from 'react-router-dom';

import type { MissionRunDto } from '../types/api';
import { StatusBadge } from './StatusBadge';
import { formatIso, formatDurationMs, shortId } from '../format';

interface Props {
  runs: MissionRunDto[];
}

export function RunsTable({ runs }: Props) {
  if (runs.length === 0) {
    return (
      <div className="empty-state">
        <p>No mission runs yet.</p>
        <p className="faint">
          Click <code>+ Create demo failed run</code> to generate the
          deterministic <code>TAKEOFF_TIMEOUT</code> demo.
        </p>
      </div>
    );
  }

  return (
    <table className="data">
      <thead>
        <tr>
          <th>Run</th>
          <th>Started</th>
          <th>Mission</th>
          <th>Vehicle</th>
          <th>Source</th>
          <th>Status</th>
          <th>Duration</th>
        </tr>
      </thead>
      <tbody>
        {runs.map((run) => {
          const duration =
            run.endedAt != null
              ? formatDurationMs(
                  Date.parse(run.endedAt) - Date.parse(run.startedAt),
                )
              : '—';
          return (
            <tr key={run.id} className="clickable">
              <td className="mono">
                <Link to={`/runs/${run.id}`} className="short-id">
                  {shortId(run.id)}
                </Link>
              </td>
              <td className="mono">{formatIso(run.startedAt)}</td>
              <td>{run.mission}</td>
              <td>{run.vehicle}</td>
              <td className="mono">{run.source}</td>
              <td>
                <StatusBadge
                  status={run.status}
                  failureReason={run.failureReason}
                />
              </td>
              <td className="mono muted">{duration}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

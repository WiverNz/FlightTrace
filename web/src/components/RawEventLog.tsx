import { useMemo, useState } from 'react';

import type { NormalizedEventDto } from '../types/api';
import { formatTimeOfDay, shortId } from '../format';

interface Props {
  events: NormalizedEventDto[];
}

export function RawEventLog({ events }: Props) {
  const [filter, setFilter] = useState('');

  const filtered = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) return events;
    return events.filter((e) => e.kind.toLowerCase().includes(needle));
  }, [events, filter]);

  return (
    <section className="section">
      <header className="section-header">
        <h2>Raw event log ({events.length})</h2>
        <div className="raw-event-controls">
          <input
            type="text"
            placeholder="filter by kind…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            aria-label="filter events by kind"
          />
          <span className="muted">{filtered.length} match</span>
        </div>
      </header>
      <div className="section-body flush">
        <div className="raw-event-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Time</th>
                <th>Event id</th>
                <th>Kind</th>
                <th>Payload</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((event) => (
                <tr key={event.id}>
                  <td className="mono">{formatTimeOfDay(event.at)}</td>
                  <td className="mono muted">{shortId(event.id)}</td>
                  <td className="mono">{event.kind}</td>
                  <td className="mono">{stringifyPayload(event)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {filtered.length === 0 && (
            <p className="muted" style={{ padding: '12px 16px' }}>
              No events match the current filter.
            </p>
          )}
        </div>
      </div>
    </section>
  );
}

function stringifyPayload(event: NormalizedEventDto): string {
  // Drop the envelope fields and the kind discriminator so the payload column
  // only shows the variant-specific data.
  const {
    id: _id,
    runId: _runId,
    at: _at,
    kind: _kind,
    ...payload
  } = event;
  return Object.keys(payload).length === 0 ? '—' : JSON.stringify(payload);
}

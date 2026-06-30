import type { EvidenceDto, NormalizedEventDto } from '../types/api';
import { formatRelativeTime, formatTimeOfDay, shortId } from '../format';

interface Props {
  evidence: EvidenceDto[];
  events: NormalizedEventDto[];
  startedAt: string | null;
}

export function EvidenceList({ evidence, events, startedAt }: Props) {
  if (evidence.length === 0) {
    return <p className="muted">No evidence recorded.</p>;
  }

  return (
    <ul className="evidence-list">
      {evidence.map((item, index) => (
        <li key={index}>
          <span className="kind">{kindLabel(item.kind)}</span>
          <span className="body">
            {renderBody(item, events, startedAt)}
          </span>
        </li>
      ))}
    </ul>
  );
}

function renderBody(
  item: EvidenceDto,
  events: NormalizedEventDto[],
  startedAt: string | null,
): React.ReactNode {
  switch (item.kind) {
    case 'Note':
      return <span className="evidence-primary">{item.text}</span>;
    case 'Event': {
      const event = events.find((candidate) => candidate.id === item.eventId);
      return (
        <>
          <span className="evidence-primary">
            {event ? describeEvent(event) : 'Recorded mission event'}
          </span>
          <span className="evidence-technical">
            event{' '}
            <code className="mono" title={item.eventId}>
              {shortId(item.eventId)}
            </code>
            {event && startedAt
              ? ` · ${formatRelativeTime(event.at, startedAt)}`
              : ''}
          </span>
        </>
      );
    }
    case 'Sample':
      return (
        <>
          <span className="evidence-primary">
            {fieldLabel(item.field)}: <strong>{formatSampleValue(item)}</strong>
          </span>
          <span className="evidence-technical">
            sample ·{' '}
            <code className="mono">
              {startedAt
                ? formatRelativeTime(item.at, startedAt)
                : formatTimeOfDay(item.at)}
            </code>
          </span>
        </>
      );
    case 'Statustext':
      return (
        <>
          <span className="evidence-primary">“{item.text}”</span>
          <span className="evidence-technical">
            vehicle STATUSTEXT ·{' '}
            <code className="mono">
              {startedAt
                ? formatRelativeTime(item.at, startedAt)
                : formatTimeOfDay(item.at)}
            </code>
          </span>
        </>
      );
  }
}

function kindLabel(kind: EvidenceDto['kind']): string {
  switch (kind) {
    case 'Event':
      return 'Mission event';
    case 'Sample':
      return 'Telemetry';
    case 'Statustext':
      return 'Vehicle warning';
    case 'Note':
      return 'Observation';
  }
}

function describeEvent(event: NormalizedEventDto): string {
  switch (event.kind) {
    case 'AltitudeUpdated':
      return `Altitude reached ${event.altitudeM.toFixed(1)} m`;
    case 'VehicleConnected':
      return 'Vehicle connected';
    case 'HeartbeatReceived':
      return 'Vehicle heartbeat received';
    case 'ArmedChanged':
      return event.armed ? 'Vehicle armed' : 'Vehicle disarmed';
    case 'ModeChanged':
      return `Flight mode changed to ${event.mode}`;
    case 'CommandAccepted':
      return `Command accepted: ${event.command}`;
    case 'CommandRejected':
      return `Command rejected: ${event.command} (${event.reason})`;
    case 'PositionUpdated':
      return `Position updated to ${event.lat.toFixed(5)}, ${event.lon.toFixed(5)}`;
    case 'BatteryUpdated':
      return `Battery at ${event.remainingPct.toFixed(0)}%`;
    case 'WaypointChanged':
      return `Advanced to waypoint ${event.index}`;
    case 'TelemetryGapDetected':
      return `Telemetry was interrupted for ${event.gapSec.toFixed(1)} s`;
    case 'StatustextReceived':
      return `Vehicle reported “${event.text}”`;
    case 'AssertionPassed':
      return `Check passed: ${event.assertionName}`;
    case 'AssertionFailed':
      return `Check failed: ${event.assertionName}`;
    case 'MissionFailed':
      return `Mission failed: ${event.reason}`;
  }
}

function fieldLabel(field: string): string {
  return field === 'altitude_m' ? 'Maximum altitude observed' : field;
}

function formatSampleValue(item: Extract<EvidenceDto, { kind: 'Sample' }>) {
  return item.field === 'altitude_m' ? `${item.value} m` : item.value;
}

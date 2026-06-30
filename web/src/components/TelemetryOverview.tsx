import type { TelemetrySampleDto } from '../types/api';
import { fmtNumber, formatTimeOfDay } from '../format';

interface Props {
  samples: TelemetrySampleDto[];
}

export function TelemetryOverview({ samples }: Props) {
  if (samples.length === 0) {
    return <p className="muted">No telemetry samples for this run.</p>;
  }

  const altitudes = samples.map((s) => s.altitudeM);
  const max = Math.max(...altitudes);
  const last = samples[samples.length - 1]!;
  const first = samples[0]!;

  return (
    <div className="telemetry-overview">
      <div className="sparkline-wrap">
        <h3>Altitude (m)</h3>
        <Sparkline values={altitudes} />
        <div className="stat">
          <span>max</span>
          <span className="value">{fmtNumber(max)} m</span>
        </div>
        <div className="stat">
          <span>last</span>
          <span className="value">{fmtNumber(last.altitudeM)} m</span>
        </div>
        <div className="stat">
          <span>samples</span>
          <span className="value">{samples.length}</span>
        </div>
        <div className="stat">
          <span>window</span>
          <span className="value">
            {formatTimeOfDay(first.at)} → {formatTimeOfDay(last.at)}
          </span>
        </div>
      </div>

      <div className="telemetry-table-wrap">
        <table className="data">
          <thead>
            <tr>
              <th>Time</th>
              <th>Alt (m)</th>
              <th>V-speed (m/s)</th>
              <th>Mode</th>
              <th>Armed</th>
              <th>Battery %</th>
            </tr>
          </thead>
          <tbody>
            {samples.map((s, i) => (
              <tr key={i}>
                <td className="mono">{formatTimeOfDay(s.at)}</td>
                <td className="mono">{fmtNumber(s.altitudeM)}</td>
                <td className="mono">{fmtNumber(s.verticalSpeedMps)}</td>
                <td className="mono">{s.mode}</td>
                <td className="mono">{s.armed ? 'yes' : 'no'}</td>
                <td className="mono">{fmtNumber(s.batteryRemainingPct, 1)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

interface SparkProps {
  values: number[];
  width?: number;
  height?: number;
}

function Sparkline({ values, width = 220, height = 56 }: SparkProps) {
  if (values.length === 0) {
    return null;
  }
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const stepX = values.length > 1 ? width / (values.length - 1) : width;
  const points = values
    .map((v, i) => {
      const x = i * stepX;
      const y = height - ((v - min) / span) * height;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(' ');

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      role="img"
      aria-label="altitude sparkline"
    >
      <polyline
        fill="none"
        stroke="var(--color-accent)"
        strokeWidth={1.5}
        points={points}
      />
    </svg>
  );
}
